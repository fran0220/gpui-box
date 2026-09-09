/* Native Seatbelt launcher with mutually supervising processes. This is a
 * candidate backend: the native tests must pass on each supported macOS lane.
 * CPU/file/fd limits are kernel rlimits. Physical footprint is an enforced
 * 256 MiB watchdog budget sampled every 25ms, NOT an allocation-time hard cap.
 */
#include <errno.h>
#include <libproc.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

static volatile sig_atomic_t stopping;
static void stop(int signal) { (void)signal; stopping = 1; }
static int limit(int resource, rlim_t amount) {
    struct rlimit value = { amount, amount };
    return setrlimit(resource, &value);
}
static int status_code(int status) {
    return WIFEXITED(status) ? WEXITSTATUS(status) : 128 + WTERMSIG(status);
}
static int same_process(pid_t pid, const struct proc_bsdinfo *identity) {
    struct proc_bsdinfo current;
    return proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &current, sizeof current) == (int)sizeof current &&
        current.pbi_start_tvsec == identity->pbi_start_tvsec &&
        current.pbi_start_tvusec == identity->pbi_start_tvusec;
}
int main(int argc, char **argv) {
    if (argc < 3) { fputs("usage: macos-launch PROFILE EXECUTABLE [ARGS]\n", stderr); return 125; }
    struct sigaction action = {0}; action.sa_handler = stop;
    sigaction(SIGTERM, &action, NULL); sigaction(SIGINT, &action, NULL);
    int announced[2], acknowledged[2];
    if (pipe(announced) || pipe(acknowledged)) return 125;
    pid_t owner = getpid(), parent = getppid();
    pid_t watcher = fork();
    if (watcher < 0) return 125;
    if (watcher == 0) {
        close(announced[0]); close(acknowledged[1]);
        int gate[2]; if (pipe(gate)) _exit(125);
        pid_t worker = fork();
        if (worker < 0) _exit(125);
        if (worker == 0) {
            close(gate[1]); close(announced[1]); close(acknowledged[0]);
            char ready;
            if (read(gate[0], &ready, 1) != 1 || ready != 'G') _exit(125);
            close(gate[0]);
            /* No supervisor handles reach untrusted code. */
            int maxfd = getdtablesize();
            for (int fd = 3; fd < maxfd; fd++) close(fd);
            if (limit(RLIMIT_CPU, 30) || limit(RLIMIT_FSIZE, 1048576) ||
                limit(RLIMIT_NOFILE, 64) || limit(RLIMIT_CORE, 0)) _exit(125);
            char **args = calloc((size_t)argc + 3, sizeof(char *));
            if (!args) _exit(125);
            args[0] = "/usr/bin/sandbox-exec"; args[1] = "-p"; args[2] = argv[1];
            for (int i = 2; i < argc; i++) args[i + 1] = argv[i];
            execv(args[0], args);
            perror("sandbox-exec"); _exit(125);
        }
        close(gate[0]);
        /* Worker cannot execute until both supervisors know its identity. If
         * either dies during startup, the gate closes and the worker exits. */
        char ready;
        if (write(announced[1], &worker, sizeof worker) != (ssize_t)sizeof worker ||
            read(acknowledged[0], &ready, 1) != 1 || ready != 'G') {
            kill(worker, SIGKILL); waitpid(worker, NULL, 0); _exit(125);
        }
        close(announced[1]); close(acknowledged[0]);
        if (write(gate[1], "G", 1) != 1) { kill(worker, SIGKILL); waitpid(worker, NULL, 0); _exit(125); }
        close(gate[1]);
        int status;
        for (;;) {
            pid_t result = waitpid(worker, &status, WNOHANG);
            if (result == worker) _exit(status_code(status));
            if (result < 0 && errno != EINTR) _exit(125);
            struct rusage_info_v4 usage = {0};
            if (stopping || getppid() != owner ||
                proc_pid_rusage(worker, RUSAGE_INFO_V4, (rusage_info_t *)&usage) != 0 ||
                usage.ri_phys_footprint > 256ULL * 1024 * 1024) {
                kill(worker, SIGKILL);
                while (waitpid(worker, &status, 0) < 0 && errno == EINTR) {}
                _exit(137);
            }
            usleep(25000);
        }
    }
    close(announced[1]); close(acknowledged[0]);
    pid_t worker = -1; struct proc_bsdinfo identity;
    if (read(announced[0], &worker, sizeof worker) != (ssize_t)sizeof worker ||
        proc_pidinfo(worker, PROC_PIDTBSDINFO, 0, &identity, sizeof identity) != (int)sizeof identity) {
        close(acknowledged[1]); kill(watcher, SIGTERM); waitpid(watcher, NULL, 0); return 125;
    }
    close(announced[0]);
    if (write(acknowledged[1], "G", 1) != 1) stopping = 1;
    close(acknowledged[1]);
    int status;
    for (;;) {
        pid_t result = waitpid(watcher, &status, WNOHANG);
        if (result == watcher) {
            if (same_process(worker, &identity)) kill(worker, SIGKILL);
            return status_code(status);
        }
        if (result < 0 && errno != EINTR) return 125;
        if (stopping || getppid() != parent) {
            if (same_process(worker, &identity)) kill(worker, SIGKILL);
            kill(watcher, SIGTERM);
        }
        usleep(25000);
    }
}

#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <spawn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <unistd.h>
extern char **environ;
static void *thread(void *value) { return value; }
int main(int argc, char **argv) {
    fputs("macos-probe: entered main\n", stderr); fflush(stderr);
    if (argc > 1 && !strcmp(argv[1], "--startup")) {
        puts("ready"); return 0;
    }
    if (argc > 1 && !strcmp(argv[1], "--memory")) {
        for (int i = 0; i < 100; i++) {
            volatile char *memory = malloc(10 * 1024 * 1024);
            if (!memory) return 2;
            for (int j = 0; j < 10 * 1024 * 1024; j += 4096) memory[j] = 1;
            usleep(5000);
        }
        sleep(5); return 3;
    }
    if (argc > 1 && !strcmp(argv[1], "--wait")) {
        printf("%d\n", getpid()); fflush(stdout); sleep(30); return 0;
    }
    int inherited = 0;
    for (int fd = 3; fd < 64; fd++) if (fcntl(fd, F_GETFD) >= 0) inherited++;
    fputs("macos-probe: filesystem\n", stderr);
    int file = open("blocked-write", O_WRONLY | O_CREAT, 0600), writes = file < 0;
    if (file >= 0) close(file);
    file = open("/etc/passwd", O_RDONLY); int reads = file < 0;
    if (file >= 0) close(file);
    file = argc > 1 ? open(argv[1], O_RDONLY) : -1;
    int host_reads = argc > 1 && file < 0;
    if (file >= 0) close(file);
    fputs("macos-probe: network\n", stderr);
    int network = socket(AF_INET, SOCK_STREAM, 0) < 0;
    fputs("macos-probe: fork\n", stderr);
    pid_t child = fork();
    if (!child) _exit(0);
    int processes = child < 0;
    if (child > 0) waitpid(child, NULL, 0);
    fputs("macos-probe: spawn\n", stderr);
    char *args[] = { "/usr/bin/true", NULL };
    int spawn_denied = posix_spawn(&child, args[0], NULL, NULL, args, environ) != 0;
    if (!spawn_denied) waitpid(child, NULL, 0);
    fputs("macos-probe: threads\n", stderr);
    pthread_t worker; void *result = NULL;
    int threads = pthread_create(&worker, NULL, thread, (void *)7) == 0;
    if (threads) { pthread_join(worker, &result); threads = result == (void *)7; }
    struct rlimit cpu, files, bytes;
    getrlimit(RLIMIT_CPU, &cpu); getrlimit(RLIMIT_NOFILE, &files); getrlimit(RLIMIT_FSIZE, &bytes);
    printf("{\"writes\":%d,\"reads\":%d,\"hostReads\":%d,\"inherited\":%d,\"network\":%d,\"fork\":%d,\"spawn\":%d,\"threads\":%d,\"cpu\":%llu,\"files\":%llu,\"bytes\":%llu}\n", writes, reads, host_reads, inherited, network, processes, spawn_denied, threads, (unsigned long long)cpu.rlim_max, (unsigned long long)files.rlim_max, (unsigned long long)bytes.rlim_max);
    return 0;
}

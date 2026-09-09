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
    int file = open("blocked-write", O_WRONLY | O_CREAT, 0600), writes = file < 0;
    if (file >= 0) close(file);
    file = open("/etc/passwd", O_RDONLY); int reads = file < 0;
    if (file >= 0) close(file);
    int network = socket(AF_INET, SOCK_STREAM, 0) < 0;
    pid_t child = fork();
    if (!child) _exit(0);
    int processes = child < 0;
    if (child > 0) waitpid(child, NULL, 0);
    char *args[] = { "/usr/bin/true", NULL };
    int spawn_denied = posix_spawn(&child, args[0], NULL, NULL, args, environ) != 0;
    if (!spawn_denied) waitpid(child, NULL, 0);
    pthread_t worker; void *result = NULL;
    int threads = pthread_create(&worker, NULL, thread, (void *)7) == 0;
    if (threads) { pthread_join(worker, &result); threads = result == (void *)7; }
    struct rlimit cpu, files, bytes;
    getrlimit(RLIMIT_CPU, &cpu); getrlimit(RLIMIT_NOFILE, &files); getrlimit(RLIMIT_FSIZE, &bytes);
    printf("{\"writes\":%d,\"reads\":%d,\"network\":%d,\"fork\":%d,\"spawn\":%d,\"threads\":%d,\"cpu\":%llu,\"files\":%llu,\"bytes\":%llu}\n", writes, reads, network, processes, spawn_denied, threads, cpu.rlim_max, files.rlim_max, bytes.rlim_max);
    return 0;
}

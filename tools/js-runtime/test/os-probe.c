#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <sched.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <unistd.h>

static void *thread_probe(void *expected_pid) {
    return (void *)(long)(getpid() == (long)expected_pid);
}

// Deliberately bypass every JS API and test the OS boundary itself.
int main(void) {
    struct rlimit memory, cpu, files;
    getrlimit(RLIMIT_AS, &memory);
    getrlimit(RLIMIT_CPU, &cpu);
    getrlimit(RLIMIT_NOFILE, &files);
    int fd = open("/app/should-not-exist", O_WRONLY | O_CREAT, 0600);
    int readonly = fd == -1 && errno == EROFS;
    if (fd >= 0) close(fd);
    fd = open("/etc/passwd", O_RDONLY);
    int hidden = fd == -1 && errno == ENOENT;
    if (fd >= 0) close(fd);
    int sock = socket(AF_INET, SOCK_STREAM, 0);
    int network = sock == -1 && errno == EPERM;
    if (sock >= 0) close(sock);
    int pid = fork();
    if (pid == 0) _exit(99);
    int processes = pid == -1 && errno == EPERM;
    long cloned = syscall(SYS_clone, CLONE_VM | SIGCHLD, 0, 0, 0, 0);
    if (cloned == 0) _exit(99);
    int clone_denied = cloned == -1 && errno == EPERM;
    cloned = syscall(SYS_clone3, NULL, 0);
    int clone3_denied = cloned == -1 && errno == ENOSYS;
    cloned = syscall(SYS_clone, CLONE_THREAD | SIGCHLD, 0, 0, 0, 0);
    int thread_bypass = cloned == -1 && errno == EINVAL;
    pthread_t thread;
    void *same_process = NULL;
    int thread_created = pthread_create(&thread, NULL, thread_probe, (void *)(long)getpid()) == 0;
    if (thread_created) pthread_join(thread, &same_process);
    int threads = thread_created && same_process == (void *)1;
    void *large = malloc(3ULL * 1024 * 1024 * 1024);
    int allocation = large == NULL;
    free(large);
    printf("{\"readonly\":%d,\"hidden\":%d,\"network\":%d,\"processes\":%d,\"clone\":%d,\"clone3\":%d,\"thread_bypass\":%d,\"threads\":%d,\"allocation\":%d,\"memory\":%llu,\"cpu\":%llu,\"files\":%llu}\n",
        readonly, hidden, network, processes, clone_denied, clone3_denied, thread_bypass, threads, allocation,
        (unsigned long long)memory.rlim_max, (unsigned long long)cpu.rlim_max,
        (unsigned long long)files.rlim_max);
    return 0;
}

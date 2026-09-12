/* GPUI Box Windows sandbox. Build instructions: ../WINDOWS.md.
 * Windows 10+; fail closed on any missing containment primitive.
 * The trusted host supplies private copies, never host source directories.
 */
#define _WIN32_WINNT 0x0A00
#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif
#include <winsock2.h>
#include <windows.h>
#include <rpc.h>
#include <userenv.h>
#include <sddl.h>
#include <aclapi.h>
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>
#include <stdint.h>
#include <objbase.h>
#include <shlobj.h>

#define MEMORY_LIMIT ((SIZE_T)256 * 1024 * 1024)
#define CPU_SECONDS 30
#define CPU_RATE 2500
#define PATH_CAP 32768

static void profile_root(const wchar_t *name, wchar_t *path);

#ifndef GPUI_SANDBOX_PROBE
static const wchar_t *owned_profile;
static HANDLE owned_job, owned_worker;
static DWORD delete_profile(const wchar_t *name) {
    wchar_t directory[PATH_CAP];
    profile_root(name, directory);
    HRESULT result;
    for (int attempt = 0;; attempt++) {
        result = DeleteAppContainerProfile(name);
        if (SUCCEEDED(result) || result == HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND) ||
            result == HRESULT_FROM_WIN32(ERROR_PATH_NOT_FOUND)) {
            if (GetFileAttributesW(directory) == INVALID_FILE_ATTRIBUTES) {
                DWORD error = GetLastError();
                if (error == ERROR_FILE_NOT_FOUND || error == ERROR_PATH_NOT_FOUND) return 0;
                result = HRESULT_FROM_WIN32(error);
            } else {
                // Successful API return need not mean profile storage has
                // vanished. Recheck after worker handle release, boundedly.
                result = HRESULT_FROM_WIN32(ERROR_BUSY);
            }
        }
        if (attempt == 20 || (result != HRESULT_FROM_WIN32(ERROR_SHARING_VIOLATION) &&
            result != E_ACCESSDENIED && result != HRESULT_FROM_WIN32(ERROR_BUSY))) break;
        Sleep(100); // job termination can release profile handles asynchronously
    }
    fprintf(stderr, "Windows sandbox: DeleteAppContainerProfile failed (0x%08lx)\n", (DWORD)result);
    return 125;
}
static DWORD cleanup_owned(void) {
    if (owned_job) { CloseHandle(owned_job); owned_job = NULL; }
    if (owned_worker) {
        WaitForSingleObject(owned_worker, INFINITE);
        CloseHandle(owned_worker); owned_worker = NULL;
    }
    if (owned_profile) {
        const wchar_t *name = owned_profile;
        owned_profile = NULL;
        return delete_profile(name);
    }
    return 0;
}
#endif
static void fail(const char *operation) {
    fprintf(stderr, "Windows sandbox: %s failed (%lu)\n", operation, GetLastError());
#ifndef GPUI_SANDBOX_PROBE
    cleanup_owned();
#endif
    ExitProcess(125);
}
#define CHECK(expr) do { if (!(expr)) fail(#expr); } while (0)

// Resolve the host's known folder, then bound every profile ACL/existence
// operation to the freshly generated moniker. GetAppContainerFolderPath's
// identity-dependent result is not authority to mutate arbitrary host paths.
static void profile_root(const wchar_t *name, wchar_t *path) {
    UUID uuid;
    CHECK(wcslen(name) == 44 && !wcsncmp(name, L"gpui-js-", 8) &&
        UuidFromStringW((RPC_WSTR)(name + 8), &uuid) == RPC_S_OK);
    PWSTR local = NULL;
    CHECK(SUCCEEDED(SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, NULL, &local)));
    CHECK(wcslen(local) + wcslen(name) + 12 < PATH_CAP);
    CHECK(swprintf(path, PATH_CAP, L"%ls\\Packages\\%ls", local, name) > 0);
    CoTaskMemFree(local);
}

#ifdef GPUI_SANDBOX_PROBE
/* Native adversarial probe: no Node permission model can mask OS failures. */
static void check_query(LONG status, const char *operation) {
    if (status == 0) return;
    fprintf(stderr, "Windows sandbox probe: %s failed (NTSTATUS 0x%08lx)\n", operation, (DWORD)status);
    ExitProcess(125);
}

// FileInternalInformation needs no specific access rights. Unlike the Win32
// aggregate metadata query (which includes FILE_READ_ATTRIBUTES), this also
// identifies data-only handles. Query the volume serial separately; never
// reopen a path, elevate the handle, or skip an access-denied disk handle.
// https://learn.microsoft.com/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_internal_information
static void disk_identity(HANDLE handle, wchar_t *identity) {
    typedef struct { union { LONG status; PVOID pointer; }; ULONG_PTR information; } io_status;
    typedef LONG (NTAPI *query_fn)(HANDLE, io_status *, PVOID, ULONG, ULONG);
    HMODULE ntdll = GetModuleHandleW(L"ntdll.dll");
    query_fn query_file = (query_fn)(void *)GetProcAddress(ntdll, "NtQueryInformationFile");
    query_fn query_volume = (query_fn)(void *)GetProcAddress(ntdll, "NtQueryVolumeInformationFile");
    CHECK(query_file && query_volume);
    io_status status;
    LARGE_INTEGER index;
    struct {
        LARGE_INTEGER creation_time;
        ULONG serial, label_length;
        BOOLEAN supports_objects;
        WCHAR label[PATH_CAP];
    } volume;
    check_query(query_file(handle, &status, &index, sizeof(index), 6), "FileInternalInformation");
    check_query(query_volume(handle, &status, &volume, sizeof(volume), 1), "FileFsVolumeInformation");
    swprintf(identity, 64, L"%08lx:%08lx:%08lx", volume.serial, (DWORD)index.HighPart, index.LowPart);
}

static void check_disk_handles(const wchar_t *sentinel) {
    for (uintptr_t value = 4; value < 65536; value += 4) {
        HANDLE handle = (HANDLE)value;
        if (GetFileType(handle) != FILE_TYPE_DISK) continue;
        DWORD flags;
        CHECK(GetHandleInformation(handle, &flags));
        wchar_t identity[64];
        disk_identity(handle, identity);
        fprintf(stderr, "Windows sandbox probe: disk handle=%llu flags=%lu id=%ls\n",
            (unsigned long long)value, flags, identity);
        // Windows opens its own non-inherited cwd/image handles. Detect the
        // host sentinel by file identity even if a duplicate cleared INHERIT.
        CHECK(wcscmp(identity, sentinel) != 0);
        CHECK(!(flags & HANDLE_FLAG_INHERIT));
    }
}

int wmain(int argc, wchar_t **argv) {
    CHECK(argc >= 3);
    if (!wcscmp(argv[1], L"--file-id")) {
        HANDLE file = CreateFileW(argv[2], FILE_READ_ATTRIBUTES, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            NULL, OPEN_EXISTING, 0, NULL);
        CHECK(file != INVALID_HANDLE_VALUE);
        BY_HANDLE_FILE_INFORMATION info;
        CHECK(GetFileInformationByHandle(file, &info));
        printf("%08lx:%08lx:%08lx\n", info.dwVolumeSerialNumber, info.nFileIndexHigh, info.nFileIndexLow);
        CloseHandle(file); return 0;
    }
    if (!wcscmp(argv[1], L"--leak-check")) {
        check_disk_handles(argv[2]); return 0;
    }
    if (!wcscmp(argv[1], L"--handle-control")) {
        CHECK(argc == 5);
        // A real data-only handle, without FILE_READ_ATTRIBUTES. The controls
        // must fail on identity even with INHERIT cleared, and on INHERIT even
        // for a different file. No host ACL is changed to manufacture denial.
        HANDLE file = CreateFileW(argv[3], FILE_READ_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            NULL, OPEN_EXISTING, 0, NULL);
        CHECK(file != INVALID_HANDLE_VALUE);
        BY_HANDLE_FILE_INFORMATION info;
        CHECK(!GetFileInformationByHandle(file, &info) && GetLastError() == ERROR_ACCESS_DENIED);
        CHECK(SetHandleInformation(file, HANDLE_FLAG_INHERIT, _wtoi(argv[4]) ? HANDLE_FLAG_INHERIT : 0));
        CHECK((uintptr_t)file < 65536);
        check_disk_handles(argv[2]);
        CloseHandle(file); return 0;
    }
    if (!wcscmp(argv[1], L"--connect-control")) {
        WSADATA wsa;
        CHECK(WSAStartup(MAKEWORD(2, 2), &wsa) == 0);
        SOCKET connection = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
        CHECK(connection != INVALID_SOCKET);
        struct sockaddr_in address = {0};
        address.sin_family = AF_INET;
        address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
        address.sin_port = htons((u_short)_wtoi(argv[2]));
        CHECK(connect(connection, (struct sockaddr *)&address, sizeof(address)) == 0);
        closesocket(connection); WSACleanup();
        puts("native TCP positive control connected"); return 0;
    }
    if (!wcscmp(argv[1], L"--profile-exists")) {
        wchar_t folder[PATH_CAP];
        profile_root(argv[2], folder);
        DWORD attributes = GetFileAttributesW(folder);
        BOOL exists = attributes != INVALID_FILE_ATTRIBUTES;
        CHECK(exists || GetLastError() == ERROR_FILE_NOT_FOUND || GetLastError() == ERROR_PATH_NOT_FOUND);
        fprintf(stderr, "Windows sandbox probe: profile=%ls folder=%ls attributes=0x%lx\n", argv[2], folder, attributes);
        puts(exists ? "true" : "false"); return 0;
    }
    if (!wcscmp(argv[1], L"--acl")) {
        PSECURITY_DESCRIPTOR descriptor = NULL;
        SECURITY_INFORMATION requested = OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
        DWORD error = GetNamedSecurityInfoW(argv[2], SE_FILE_OBJECT, requested, NULL, NULL, NULL, NULL, &descriptor);
        if (error) { SetLastError(error); fail("GetNamedSecurityInfoW"); }
        LPWSTR text = NULL;
        CHECK(ConvertSecurityDescriptorToStringSecurityDescriptorW(descriptor, SDDL_REVISION_1, requested, &text, NULL));
        wprintf(L"%ls\n", text);
        LocalFree(text); LocalFree(descriptor);
        return 0;
    }
    if (!wcscmp(argv[1], L"spin")) {
        volatile uint64_t counter = 0;
        puts("spinning"); fflush(stdout);
        for (;;) counter++;
    }
    if (!wcscmp(argv[1], L"memory")) {
        unsigned allocations = 0;
        while (VirtualAlloc(NULL, 8 * 1024 * 1024, MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE)) {
            if (++allocations > 64) return 2;
        }
        printf("{\"allocations\":%u}\n", allocations);
        return allocations > 0 && allocations < 32 ? 0 : 3;
    }
    HANDLE token;
    DWORD value, size;
    CHECK(OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token));
    CHECK(GetTokenInformation(token, TokenIsAppContainer, &value, sizeof(value), &size));
    CHECK(value == 1);
    GetTokenInformation(token, TokenCapabilities, NULL, 0, &size);
    TOKEN_GROUPS *groups = malloc(size);
    CHECK(groups && GetTokenInformation(token, TokenCapabilities, groups, size, &size));
    CHECK(groups->GroupCount == 0);
    free(groups);
    GetTokenInformation(token, TokenAppContainerSid, NULL, 0, &size);
    TOKEN_APPCONTAINER_INFORMATION *container = malloc(size);
    CHECK(container && GetTokenInformation(token, TokenAppContainerSid, container, size, &size));
    LPWSTR package_sid = NULL, profile_folder = NULL;
    CHECK(ConvertSidToStringSidW(container->TokenAppContainer, &package_sid));
    fprintf(stderr, "Windows sandbox probe: pid=%lu packageSID=%ls tcp=127.0.0.1:%ls\n", GetCurrentProcessId(), package_sid, argv[1]);
    CHECK(SUCCEEDED(GetAppContainerFolderPath(package_sid, &profile_folder)));
    const wchar_t *environment_keys[] = {L"LOCALAPPDATA", L"TEMP", L"TMP"};
    for (int i = 0; i < 3; i++) {
        wchar_t value[PATH_CAP];
        DWORD length = GetEnvironmentVariableW(environment_keys[i], value, PATH_CAP);
        CHECK(length && length < PATH_CAP - 32);
        size_t prefix = wcslen(profile_folder);
        fprintf(stderr, "Windows sandbox probe: env key=%ls path=%ls attributes=0x%lx profile=%ls\n",
            environment_keys[i], value, GetFileAttributesW(value), profile_folder);
        CHECK(!_wcsnicmp(value, profile_folder, prefix) &&
            (!value[prefix] || value[prefix] == L'\\'));
        CHECK(i == 0 ? value[prefix] == 0 : !_wcsicmp(value + prefix, L"\\Temp"));
        CHECK(GetFileAttributesW(value) != INVALID_FILE_ATTRIBUTES);
        wcscat(value, L"\\forbidden-environment.txt");
        HANDLE write = CreateFileW(value, GENERIC_WRITE, 0, NULL, CREATE_ALWAYS, 0, NULL);
        CHECK(write == INVALID_HANDLE_VALUE && GetLastError() == ERROR_ACCESS_DENIED);
    }
    wchar_t profile_file[PATH_CAP];
    CHECK(swprintf(profile_file, PATH_CAP, L"%ls\\forbidden.txt", profile_folder) > 0);
    HANDLE profile_write = CreateFileW(profile_file, GENERIC_WRITE, 0, NULL, CREATE_ALWAYS, 0, NULL);
    CHECK(profile_write == INVALID_HANDLE_VALUE && GetLastError() == ERROR_ACCESS_DENIED);
    CoTaskMemFree(profile_folder); LocalFree(package_sid); free(container);
    CloseHandle(token);
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION limits;
    JOBOBJECT_CPU_RATE_CONTROL_INFORMATION cpu;
    CHECK(QueryInformationJobObject(NULL, JobObjectExtendedLimitInformation, &limits, sizeof(limits), NULL));
    CHECK(QueryInformationJobObject(NULL, JobObjectCpuRateControlInformation, &cpu, sizeof(cpu), NULL));
    DWORD required = JOB_OBJECT_LIMIT_ACTIVE_PROCESS | JOB_OBJECT_LIMIT_PROCESS_MEMORY |
        JOB_OBJECT_LIMIT_JOB_MEMORY | JOB_OBJECT_LIMIT_PROCESS_TIME | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    CHECK((limits.BasicLimitInformation.LimitFlags & required) == required);
    CHECK(limits.BasicLimitInformation.ActiveProcessLimit == 1);
    // Expectations intentionally do not reuse the launcher's policy macros.
    CHECK(limits.ProcessMemoryLimit == 268435456 && limits.JobMemoryLimit == 268435456);
    CHECK(limits.BasicLimitInformation.PerProcessUserTimeLimit.QuadPart == 300000000LL);
    CHECK(cpu.ControlFlags == (JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP));
    CHECK(cpu.CpuRate == 2500);
    HANDLE file = CreateFileW(argv[2], GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, 0, NULL);
    CHECK(file == INVALID_HANDLE_VALUE && GetLastError() == ERROR_ACCESS_DENIED);
    file = CreateFileW(L"forbidden.txt", GENERIC_WRITE, 0, NULL, CREATE_ALWAYS, 0, NULL);
    CHECK(file == INVALID_HANDLE_VALUE && GetLastError() == ERROR_ACCESS_DENIED);
    wchar_t cwd[PATH_CAP];
    CHECK(GetCurrentDirectoryW(PATH_CAP, cwd));
    CHECK(SetNamedSecurityInfoW(cwd, SE_FILE_OBJECT, DACL_SECURITY_INFORMATION,
        NULL, NULL, NULL, NULL) == ERROR_ACCESS_DENIED);
    wchar_t self[PATH_CAP];
    CHECK(GetModuleFileNameW(NULL, self, PATH_CAP));
    STARTUPINFOW startup = { .cb = sizeof(startup) };
    PROCESS_INFORMATION process = {0};
    CHECK(!CreateProcessW(self, NULL, NULL, NULL, FALSE, 0, NULL, NULL, &startup, &process));
    CHECK(!CreateProcessW(self, NULL, NULL, NULL, FALSE, CREATE_BREAKAWAY_FROM_JOB, NULL, NULL, &startup, &process));
    // Run independent handle assertions before the network probe, so a
    // blocked connection's timeout cannot hide their native result.
    CHECK(argc >= 4);
    check_disk_handles(argv[3]);
    fputs("Windows sandbox probe: filesystem, spawn, inherited-handle assertions passed\n", stderr);
    // A listener is established by the test parent: refusal is not a closed port.
    WSADATA wsa;
    CHECK(WSAStartup(MAKEWORD(2, 2), &wsa) == 0);
    SOCKET socketHandle = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    CHECK(socketHandle != INVALID_SOCKET);
    struct sockaddr_in address = {0};
    address.sin_family = AF_INET;
    address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    address.sin_port = htons((u_short)_wtoi(argv[1]));
    CHECK(connect(socketHandle, (struct sockaddr *)&address, sizeof(address)) == SOCKET_ERROR);
    int connect_error = WSAGetLastError();
    fprintf(stderr, "Windows sandbox probe: TCP result=%d destination=127.0.0.1:%ls\n", connect_error, argv[1]);
    CHECK(connect_error == WSAEACCES); // timeout alone is not policy-drop evidence
    closesocket(socketHandle);
    socketHandle = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    CHECK(socketHandle != INVALID_SOCKET);
    address.sin_addr.s_addr = htonl(0xc0000201); // TEST-NET-1, not a live service
    address.sin_port = htons(9);
    CHECK(sendto(socketHandle, "x", 1, 0, (struct sockaddr *)&address, sizeof(address)) == SOCKET_ERROR);
    CHECK(WSAGetLastError() == WSAEACCES);
    closesocket(socketHandle);
    WSACleanup();
    puts("{\"appcontainer\":true,\"capabilities\":0,\"readonly\":true,\"hostDenied\":true,\"spawnDenied\":true,\"networkDenied\":true,\"handles\":true,\"memory\":268435456,\"cpuSeconds\":30,\"cpuRate\":2500,\"activeProcesses\":1}");
    return 0;
}
#else
static wchar_t *join(const wchar_t *root, const wchar_t *suffix) {
    size_t length = wcslen(root) + wcslen(suffix) + 2;
    CHECK(length < PATH_CAP);
    wchar_t *result = calloc(length, sizeof(wchar_t));
    CHECK(result);
    swprintf(result, length, L"%ls\\%ls", root, suffix);
    return result;
}

// Explicit protected ACLs discard inherited access (including broad package
// groups). The caller retains ownership; the unique sandbox SID gets RX only.
static void protect_tree(wchar_t *path, PACL acl) {
    DWORD attributes = GetFileAttributesW(path);
    CHECK(attributes != INVALID_FILE_ATTRIBUTES);
    CHECK(!(attributes & FILE_ATTRIBUTE_REPARSE_POINT));
    // Protect children first. Replacing a parent's inheritable ACL first can
    // revoke the host's access to not-yet-protected children; inheritable
    // grants could instead propagate through an unexamined junction.
    if (attributes & FILE_ATTRIBUTE_DIRECTORY) {
        wchar_t *pattern = join(path, L"*");
        WIN32_FIND_DATAW entry;
        HANDLE scan = FindFirstFileW(pattern, &entry);
        free(pattern);
        CHECK(scan != INVALID_HANDLE_VALUE);
        do {
            if (!wcscmp(entry.cFileName, L".") || !wcscmp(entry.cFileName, L"..")) continue;
            wchar_t *child = join(path, entry.cFileName);
            protect_tree(child, acl);
            free(child);
        } while (FindNextFileW(scan, &entry));
        CHECK(GetLastError() == ERROR_NO_MORE_FILES);
        FindClose(scan);
    }
    DWORD error = SetNamedSecurityInfoW(path, SE_FILE_OBJECT,
        DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
        NULL, NULL, acl, NULL);
    if (error) { SetLastError(error); fail("private copy ACL"); }
}

static int is_under(const wchar_t *path, const wchar_t *root) {
    size_t n = wcslen(root);
    return !_wcsnicmp(path, root, n) &&
        (path[n] == 0 || path[n] == L'\\' || path[n] == L'/');
}

static wchar_t *remap(const wchar_t *argument, const wchar_t *root,
                       const wchar_t *runtime, const wchar_t *instance) {
    const wchar_t *prefix = L"--allow-fs-read=";
    size_t offset = !wcsncmp(argument, prefix, wcslen(prefix)) ? wcslen(prefix) : 0;
    const wchar_t *value = argument + offset;
    // Node realpath canonicalizes the source roots, whereas appended entry
    // paths may still use the runner's 8.3 TEMP alias. Expand existing absolute
    // paths before matching, but preserve non-package arguments byte-for-byte.
    wchar_t expanded[PATH_CAP];
    if ((wcslen(value) > 2 && value[1] == L':' &&
         (value[2] == L'\\' || value[2] == L'/')) ||
        (value[0] == L'\\' && value[1] == L'\\')) {
        DWORD length = GetLongPathNameW(value, expanded, PATH_CAP);
        if (length && length < PATH_CAP) {
            for (wchar_t *p = expanded; *p; p++) if (*p == L'/') *p = L'\\';
            value = expanded;
        }
    }
    const wchar_t *source = NULL, *target = NULL;
    // Longest prefix wins when runtime and package are nested.
    if (is_under(value, root)) { source = root; target = L"package"; }
    if (is_under(value, runtime) && (!source || wcslen(runtime) > wcslen(source))) {
        source = runtime; target = L"runtime";
    }
    if (!source) return _wcsdup(argument);
    wchar_t *destination = join(instance, target);
    size_t length = offset + wcslen(destination) + wcslen(value + wcslen(source)) + 1;
    wchar_t *result = calloc(length, sizeof(wchar_t));
    CHECK(result);
    wcsncpy(result, argument, offset);
    wcscat(result, destination);
    wcscat(result, value + wcslen(source));
    free(destination);
    return result;
}

// Windows CRT command-line quoting, including quotes and trailing backslashes.
static void append_argument(wchar_t *command, const wchar_t *argument) {
    size_t used = wcslen(command), slashes = 0;
    CHECK(used + 2 * wcslen(argument) + 4 < PATH_CAP);
    wchar_t *out = command + used;
    if (used) *out++ = L' ';
    *out++ = L'"';
    for (const wchar_t *p = argument;; p++) {
        if (*p == L'\\') { slashes++; continue; }
        size_t count = (*p == L'"' || !*p) ? slashes * 2 : slashes;
        while (count--) *out++ = L'\\';
        slashes = 0;
        if (!*p) break;
        if (*p == L'"') *out++ = L'\\';
        *out++ = *p;
    }
    *out++ = L'"';
    *out = 0;
}

int wmain(int argc, wchar_t **argv) {
    if (argc == 3 && !wcscmp(argv[1], L"--delete-profile")) {
        UUID uuid;
        CHECK(wcslen(argv[2]) == 44 && !wcsncmp(argv[2], L"gpui-js-", 8) &&
            UuidFromStringW(argv[2] + 8, &uuid) == RPC_S_OK);
        return (int)delete_profile(argv[2]);
    }
    CHECK(argc >= 12 && !wcscmp(argv[1], L"--instance") &&
        !wcscmp(argv[3], L"--root") && !wcscmp(argv[5], L"--runtime") &&
        !wcscmp(argv[7], L"--parent") && !wcscmp(argv[9], L"--profile") && !wcscmp(argv[11], L"--"));
    wchar_t instance[PATH_CAP];
    DWORD instance_length = GetLongPathNameW(argv[2], instance, PATH_CAP);
    CHECK(instance_length && instance_length < PATH_CAP);
    for (wchar_t *p = instance; *p; p++) if (*p == L'/') *p = L'\\';
    CHECK(wcslen(instance) > 3 && GetFileAttributesW(instance) != INVALID_FILE_ATTRIBUTES);
    HANDLE parent = OpenProcess(SYNCHRONIZE, FALSE, wcstoul(argv[8], NULL, 10));
    CHECK(parent);
    CHECK(WaitForSingleObject(parent, 0) == WAIT_TIMEOUT);

    // Unpackaged AppContainers need profile provisioning, not just a SID hash.
    // The host keeps this unique name for cleanup if the helper is terminated.
    UUID uuid;
    const wchar_t *name = argv[10];
    CHECK(wcslen(name) == 44 && !wcsncmp(name, L"gpui-js-", 8) &&
        UuidFromStringW((RPC_WSTR)(name + 8), &uuid) == RPC_S_OK);
    wchar_t profile_directory[PATH_CAP];
    profile_root(name, profile_directory);
    CHECK(GetFileAttributesW(profile_directory) == INVALID_FILE_ATTRIBUTES &&
        (GetLastError() == ERROR_FILE_NOT_FOUND || GetLastError() == ERROR_PATH_NOT_FOUND));
    PSID sid = NULL;
    HRESULT result = CreateAppContainerProfile(name, name, L"GPUI Box isolated runtime", NULL, 0, &sid);
    if (FAILED(result)) { SetLastError((DWORD)result); fail("CreateAppContainerProfile"); }
    owned_profile = name;
    HANDLE token;
    DWORD size;
    CHECK(OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token));
    GetTokenInformation(token, TokenUser, NULL, 0, &size);
    TOKEN_USER *user = malloc(size);
    CHECK(user && GetTokenInformation(token, TokenUser, user, size, &size));
    LPWSTR userText = NULL, sidText = NULL;
    CHECK(ConvertSidToStringSidW(user->User.Sid, &userText));
    CHECK(ConvertSidToStringSidW(sid, &sidText));
    wchar_t sddl[1024];
    swprintf(sddl, 1024, L"D:P(A;;FA;;;SY)(A;;FA;;;%ls)(A;;GRGX;;;%ls)", userText, sidText);
    PSECURITY_DESCRIPTOR descriptor = NULL;
    CHECK(ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl, SDDL_REVISION_1, &descriptor, NULL));
    PACL acl;
    BOOL present, defaulted;
    CHECK(GetSecurityDescriptorDacl(descriptor, &present, &acl, &defaulted) && present);
    protect_tree(instance, acl);
    wchar_t *profile_path = join(profile_directory, L"AC");
    CHECK(CreateDirectoryW(profile_path, NULL) || GetLastError() == ERROR_ALREADY_EXISTS);
    wchar_t *profile_temp = join(profile_path, L"Temp");
    CHECK(CreateDirectoryW(profile_temp, NULL) || GetLastError() == ERROR_ALREADY_EXISTS);
    protect_tree(profile_directory, acl); // never an identity-dependent host folder
    LocalFree(descriptor); LocalFree(userText); LocalFree(sidText); free(user); CloseHandle(token);

    HANDLE job = CreateJobObjectW(NULL, NULL);
    CHECK(job);
    owned_job = job;
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION limits = {0};
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_ACTIVE_PROCESS |
        JOB_OBJECT_LIMIT_PROCESS_MEMORY | JOB_OBJECT_LIMIT_JOB_MEMORY |
        JOB_OBJECT_LIMIT_PROCESS_TIME | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE |
        JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
    limits.BasicLimitInformation.ActiveProcessLimit = 1;
    limits.BasicLimitInformation.PerProcessUserTimeLimit.QuadPart = CPU_SECONDS * 10000000LL;
    limits.ProcessMemoryLimit = MEMORY_LIMIT;
    limits.JobMemoryLimit = MEMORY_LIMIT;
    CHECK(SetInformationJobObject(job, JobObjectExtendedLimitInformation, &limits, sizeof(limits)));
    JOBOBJECT_CPU_RATE_CONTROL_INFORMATION cpu = {0};
    cpu.ControlFlags = JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP;
    cpu.CpuRate = CPU_RATE;
    CHECK(SetInformationJobObject(job, JobObjectCpuRateControlInformation, &cpu, sizeof(cpu)));

    STARTUPINFOEXW startup = {0};
    startup.StartupInfo.cb = sizeof(startup);
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    HANDLE inherited[3];
    DWORD standard[3] = {STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE};
    for (int i = 0; i < 3; i++) {
        HANDLE original = GetStdHandle(standard[i]);
        CHECK(GetFileType(original) == FILE_TYPE_PIPE);
        CHECK(DuplicateHandle(GetCurrentProcess(), original, GetCurrentProcess(), &inherited[i],
            0, TRUE, DUPLICATE_SAME_ACCESS));
    }
    startup.StartupInfo.hStdInput = inherited[0];
    startup.StartupInfo.hStdOutput = inherited[1];
    startup.StartupInfo.hStdError = inherited[2];
    SIZE_T bytes = 0;
    InitializeProcThreadAttributeList(NULL, 3, 0, &bytes);
    startup.lpAttributeList = malloc(bytes);
    CHECK(startup.lpAttributeList && InitializeProcThreadAttributeList(startup.lpAttributeList, 3, 0, &bytes));
    SECURITY_CAPABILITIES capabilities = {0};
    capabilities.AppContainerSid = sid;
    CHECK(UpdateProcThreadAttribute(startup.lpAttributeList, 0, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
        &capabilities, sizeof(capabilities), NULL, NULL));
    CHECK(UpdateProcThreadAttribute(startup.lpAttributeList, 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
        inherited, sizeof(inherited), NULL, NULL));
    // Atomic membership: even TerminateProcess on the helper between creation
    // and first instruction cannot orphan a worker outside the job.
    CHECK(UpdateProcThreadAttribute(startup.lpAttributeList, 0, PROC_THREAD_ATTRIBUTE_JOB_LIST,
        &job, sizeof(job), NULL, NULL));
    wchar_t *executable = join(instance, L"worker.exe");
    wchar_t *cwd = join(instance, L"package");
    wchar_t *command = calloc(PATH_CAP, sizeof(wchar_t));
    CHECK(command);
    append_argument(command, executable);
    for (int i = 12; i < argc; i++) {
        wchar_t *argument = remap(argv[i], argv[4], argv[6], instance);
        CHECK(argument);
        append_argument(command, argument);
        free(argument);
    }
    // Never inherit host secrets, NODE_OPTIONS, or loader search paths.
    // Windows documents rewriting LOCALAPPDATA/TEMP/TMP for AppContainer
    // creation. The native lane reported error 203 with all three omitted,
    // even for an invalid image. LOCALAPPDATA is the INPUT base to that
    // rewrite: passing AC here duplicated Packages/<name>/AC in the child.
    // Obtain only this known-folder bootstrap value, not the host environment.
    // TEMP/TMP remain private. No ACL grant is made to the input base.
    PWSTR local_app_data = NULL;
    CHECK(SUCCEEDED(SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, NULL, &local_app_data)));
    wchar_t windows[PATH_CAP];
    UINT length = GetWindowsDirectoryW(windows, PATH_CAP);
    CHECK(length && length < PATH_CAP);
    const wchar_t *keys[] = {L"LOCALAPPDATA", L"NODE_NO_WARNINGS", L"SystemRoot", L"TEMP", L"TMP", L"WINDIR"};
    const wchar_t *values[] = {local_app_data, L"1", windows, profile_temp, profile_temp, windows};
    size_t envSize = 1;
    for (int i = 0; i < 6; i++) envSize += wcslen(keys[i]) + wcslen(values[i]) + 2;
    wchar_t *environment = calloc(envSize, sizeof(wchar_t));
    CHECK(environment);
    size_t offset = 0;
    for (int i = 0; i < 6; i++) {
        int written = swprintf(environment + offset, envSize - offset, L"%ls=%ls", keys[i], values[i]);
        CHECK(written > 0 && (size_t)written < envSize - offset);
        offset += (size_t)written + 1;
    }
    CHECK(offset + 1 == envSize && environment[offset] == 0);
    CoTaskMemFree(local_app_data);
    free(profile_path); free(profile_temp);
    DWORD executable_attributes = GetFileAttributesW(executable);
    DWORD cwd_attributes = GetFileAttributesW(cwd);
    CHECK(executable_attributes != INVALID_FILE_ATTRIBUTES && !(executable_attributes & FILE_ATTRIBUTE_DIRECTORY));
    CHECK(cwd_attributes != INVALID_FILE_ATTRIBUTES && (cwd_attributes & FILE_ATTRIBUTE_DIRECTORY));
    HANDLE image = CreateFileW(executable, GENERIC_READ | GENERIC_EXECUTE,
        FILE_SHARE_READ | FILE_SHARE_DELETE, NULL, OPEN_EXISTING, 0, NULL);
    CHECK(image != INVALID_HANDLE_VALUE);
    CloseHandle(image);
    PROCESS_INFORMATION process = {0};
    // No console is needed: all three standard handles are explicit pipes.
    // CREATE_NO_WINDOW still requests an invisible console on Windows.
    DWORD flags = EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT | DETACHED_PROCESS;
    if (!CreateProcessW(executable, command, NULL, NULL, TRUE,
        flags, environment, cwd, &startup.StartupInfo, &process)) {
        DWORD error = GetLastError();
        fprintf(stderr, "Windows sandbox: launch exe=%ls cwd=%ls exeAttributes=0x%lx cwdAttributes=0x%lx flags=0x%lx profile=%ls env=LOCALAPPDATA,NODE_NO_WARNINGS,SystemRoot,TEMP,TMP,WINDIR\n",
            executable, cwd, executable_attributes, cwd_attributes, flags, name);
        SetLastError(error); fail("CreateProcessW");
    }
    owned_worker = process.hProcess;
    for (int i = 0; i < 3; i++) CloseHandle(inherited[i]);
    DeleteProcThreadAttributeList(startup.lpAttributeList);
    free(startup.lpAttributeList); FreeSid(sid);
    CloseHandle(process.hThread);
    HANDLE wait[] = {process.hProcess, parent};
    DWORD waited = WaitForMultipleObjects(2, wait, FALSE, INFINITE), exitCode = 125;
    CHECK(waited == WAIT_OBJECT_0 || waited == WAIT_OBJECT_0 + 1);
    if (waited == WAIT_OBJECT_0) CHECK(GetExitCodeProcess(process.hProcess, &exitCode));
    if (exitCode == 0xc0000044UL) { // STATUS_QUOTA_EXCEEDED
        FILETIME created, exited, kernel, user_time;
        CHECK(GetProcessTimes(process.hProcess, &created, &exited, &kernel, &user_time));
        unsigned long long ticks = ((unsigned long long)user_time.dwHighDateTime << 32) | user_time.dwLowDateTime;
        fprintf(stderr, "Windows sandbox: quota exit user_100ns=%llu\n", ticks);
    }
    DWORD cleanup_error = cleanup_owned(); // kills/reaps before deleting profile
    CloseHandle(parent);
    free(command); free(environment); free(executable); free(cwd);
    return cleanup_error ? (int)cleanup_error : (int)exitCode;
}
#endif

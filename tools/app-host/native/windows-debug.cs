// Developer-only transport. The name is an address, never an authorization secret.
// Windows enforces a protected owner-only DACL before opening; after the first
// bounded read we independently authenticate the client's impersonation token.
using System;
using System.ComponentModel;
using System.IO;
using System.IO.Pipes;
using System.Runtime.InteropServices;
using System.Runtime.Serialization.Json;
using System.Security.AccessControl;
using System.Security.Principal;
using System.Text;
using System.Threading.Tasks;
using System.Xml;
using Microsoft.Win32.SafeHandles;

internal static class DebugPipe
{
    const int Limit = 256 * 1024;
    const int Deadline = 5000;
    const uint DuplexOverlappedFirst = 3 | 0x40000000 | 0x00080000;
    const uint RejectRemoteClients = 8;
    static readonly UTF8Encoding Utf8 = new UTF8Encoding(false, true);

    [StructLayout(LayoutKind.Sequential)]
    struct SecurityAttributes { public int Length; public IntPtr Descriptor; public int Inherit; }
    [StructLayout(LayoutKind.Sequential)]
    struct SidAndAttributes { public IntPtr Sid; public uint Attributes; }
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern SafePipeHandle CreateNamedPipe(string name, uint openMode, uint pipeMode,
        uint instances, uint outSize, uint inSize, uint timeout, ref SecurityAttributes attributes);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern SafeFileHandle CreateFile(string name, uint access, uint share, IntPtr attributes,
        uint disposition, uint flags, IntPtr template);
    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool CreateRestrictedToken(IntPtr existing, uint flags, uint disableCount,
        IntPtr disable, uint privilegeCount, IntPtr privileges, uint restrictCount,
        ref SidAndAttributes restrict, out IntPtr token);
    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool DuplicateTokenEx(IntPtr existing, uint access, IntPtr attributes,
        int impersonationLevel, int tokenType, out IntPtr token);
    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool SetThreadToken(IntPtr thread, IntPtr token);
    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool RevertToSelf();
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool CancelIoEx(SafePipeHandle handle, IntPtr overlapped);

    static NamedPipeServerStream Create(string name, SecurityIdentifier user)
    {
        var acl = new PipeSecurity();
        acl.SetOwner(user);
        acl.SetAccessRuleProtection(true, false);
        acl.AddAccessRule(new PipeAccessRule(user, PipeAccessRights.FullControl, AccessControlType.Allow));
        var descriptor = acl.GetSecurityDescriptorBinaryForm();
        var pinned = GCHandle.Alloc(descriptor, GCHandleType.Pinned);
        try
        {
            var attributes = new SecurityAttributes {
                Length = Marshal.SizeOf(typeof(SecurityAttributes)), Descriptor = pinned.AddrOfPinnedObject(), Inherit = 0
            };
            var handle = CreateNamedPipe(name, DuplexOverlappedFirst, RejectRemoteClients, 1,
                Limit, Limit, Deadline, ref attributes);
            if (handle.IsInvalid) { int error = Marshal.GetLastWin32Error(); handle.Dispose(); throw new Win32Exception(error); }
            try { return new NamedPipeServerStream(PipeDirection.InOut, true, false, handle); }
            catch { handle.Dispose(); throw; }
        }
        finally { pinned.Free(); }
    }

    // Inspect the actual kernel object's ACL, not merely the requested descriptor.
    static string SecurityEvidence(NamedPipeServerStream pipe, SecurityIdentifier user)
    {
        var acl = pipe.GetAccessControl();
        var rules = acl.GetAccessRules(true, true, typeof(SecurityIdentifier));
        if (!acl.AreAccessRulesProtected || !user.Equals(acl.GetOwner(typeof(SecurityIdentifier))) || rules.Count != 1)
            throw new InvalidOperationException("Debug pipe ACL is not protected owner-only");
        var rule = (PipeAccessRule)rules[0];
        if (!user.Equals(rule.IdentityReference) || rule.IsInherited || rule.AccessControlType != AccessControlType.Allow ||
            rule.PipeAccessRights != PipeAccessRights.FullControl)
            throw new InvalidOperationException("Debug pipe ACL grants unexpected access");
        return "{\"ready\":true,\"security\":{\"transport\":\"windows-named-pipe\",\"owner\":\"" + user.Value +
            "\",\"protected\":true,\"allowedSids\":[\"" + user.Value + "\"],\"sddl\":\"" +
            acl.GetSecurityDescriptorSddlForm(AccessControlSections.Owner | AccessControlSections.Access) + "\"}}";
    }

    static async Task<string> ReadFrame(Stream input)
    {
        var bytes = new byte[4096];
        using (var frame = new MemoryStream())
        {
            while (true)
            {
                int count = await input.ReadAsync(bytes, 0, bytes.Length).ConfigureAwait(false);
                if (count == 0) { if (frame.Length == 0) return null; throw new IOException("Truncated debug frame"); }
                int newline = Array.IndexOf(bytes, (byte)10, 0, count);
                int take = newline < 0 ? count : newline;
                if (frame.Length + take >= Limit) throw new IOException("Debug frame exceeds limit");
                frame.Write(bytes, 0, take);
                if (newline >= 0)
                {
                    if (newline != count - 1) throw new IOException("Only one debug request is allowed");
                    return Utf8.GetString(frame.ToArray());
                }
            }
        }
    }

    static bool WaitClient(Task operation, Task<string> host, int timeout, NamedPipeServerStream pipe)
    {
        int winner = Task.WaitAny(new Task[] { host, operation }, timeout);
        if (winner == 0)
        {
            if (host.GetAwaiter().GetResult() != null) throw new InvalidOperationException("Unexpected host response");
            return false; // Host EOF cancels even an idle connection or a stalled read.
        }
        if (winner < 0)
        {
            // Retain the first-instance handle across clients (no name-squatting
            // or reconnect gap). Drain cancelled IO before reusing that handle.
            if (!CancelIoEx(pipe.SafePipeHandle, IntPtr.Zero) && Marshal.GetLastWin32Error() != 1168)
                throw new Win32Exception(Marshal.GetLastWin32Error());
            try { if (!operation.Wait(1000)) throw new InvalidOperationException("Debug IO cancellation timed out"); }
            catch (AggregateException) { } // Expected cancelled/failed IO, now complete.
            throw new TimeoutException("Debug client timed out");
        }
        operation.GetAwaiter().GetResult();
        return true;
    }

    static void Serve(string name)
    {
        var user = WindowsIdentity.GetCurrent().User;
        using (var input = Console.OpenStandardInput())
        {
            // Anonymous stdin handles can execute ReadAsync synchronously on .NET Framework.
            Task<string> host = Task.Run(() => ReadFrame(input));
            using (var pipe = Create(name, user))
            {
                Console.WriteLine(SecurityEvidence(pipe, user));
                while (true)
                {
                    var connection = Task.Factory.FromAsync(pipe.BeginWaitForConnection, pipe.EndWaitForConnection, null);
                    if (!WaitClient(connection, host, -1, pipe)) return;
                    try
                    {
                        var read = ReadFrame(pipe);
                        if (!WaitClient(read, host, Deadline, pipe)) return;
                        string request = read.GetAwaiter().GetResult();
                        if (request == null) { pipe.Disconnect(); continue; }
                        bool sameUser = false;
                        pipe.RunAsClient(delegate {
                            using (var client = WindowsIdentity.GetCurrent(true))
                                sameUser = client != null && user.Equals(client.User);
                        });
                        if (!sameUser) throw new UnauthorizedAccessException("Debug client is not the current user");
                        // Invalid client JSON must close this connection, not poison
                        // the host's trusted helper framing stream.
                        using (var json = JsonReaderWriterFactory.CreateJsonReader(Utf8.GetBytes(request), new XmlDictionaryReaderQuotas {
                            MaxDepth = 16, MaxStringContentLength = Limit, MaxArrayLength = Limit
                        })) { while (json.Read()) { } }
                        Console.WriteLine(request);
                        // Timeout here is fatal: never deliver a late host reply to a later client.
                        if (!host.Wait(Deadline)) throw new InvalidOperationException("Debug host response timed out");
                        string response = host.GetAwaiter().GetResult();
                        if (response == null) return;
                        host = Task.Run(() => ReadFrame(input));
                        byte[] bytes = Utf8.GetBytes(response + "\n");
                        if (!WaitClient(pipe.WriteAsync(bytes, 0, bytes.Length), host, Deadline, pipe)) return;
                        // DisconnectNamedPipe discards unread output. Wait for the
                        // client's EOF (evaluateDebug closes after reading) instead
                        // of using the unbounded synchronous FlushFileBuffers API.
                        var eof = pipe.ReadAsync(new byte[1], 0, 1);
                        if (!WaitClient(eof, host, Deadline, pipe)) return;
                        if (eof.GetAwaiter().GetResult() != 0) throw new IOException("Multiple debug requests");
                    }
                    catch (IOException) { }
                    catch (TimeoutException) { }
                    catch (UnauthorizedAccessException) { }
                    catch (System.Security.SecurityException) { }
                    catch (XmlException) { }
                    catch (DecoderFallbackException) { }
                    pipe.Disconnect();
                }
            }
        }
    }

    // Native negative-access test: a token restricted to Everyone must fail the
    // second Windows access check against our user-only DACL. No administrator needed.
    static void ProbeDenied(string name)
    {
        var sid = new SecurityIdentifier(WellKnownSidType.WorldSid, null);
        var bytes = new byte[sid.BinaryLength]; sid.GetBinaryForm(bytes, 0);
        var pinned = GCHandle.Alloc(bytes, GCHandleType.Pinned);
        IntPtr token = IntPtr.Zero, impersonation = IntPtr.Zero;
        try
        {
            var restricted = new SidAndAttributes { Sid = pinned.AddrOfPinnedObject(), Attributes = 0 };
            using (var identity = WindowsIdentity.GetCurrent())
                if (!CreateRestrictedToken(identity.Token, 1, 0, IntPtr.Zero, 0, IntPtr.Zero, 1, ref restricted, out token))
                    throw new Win32Exception(Marshal.GetLastWin32Error());
            // CreateRestrictedToken preserves the primary token's type. A thread
            // must receive an impersonation token, not that primary token.
            if (!DuplicateTokenEx(token, 0x0c, IntPtr.Zero, 2, 2, out impersonation))
                throw new Win32Exception(Marshal.GetLastWin32Error());
            if (!SetThreadToken(IntPtr.Zero, impersonation)) throw new Win32Exception(Marshal.GetLastWin32Error());
            int error;
            bool opened;
            try
            {
                using (var handle = CreateFile(name, 0xc0000000, 0, IntPtr.Zero, 3, 0, IntPtr.Zero))
                { error = Marshal.GetLastWin32Error(); opened = !handle.IsInvalid; }
            }
            finally { if (!RevertToSelf()) throw new Win32Exception(Marshal.GetLastWin32Error()); }
            if (opened || error != 5) throw new InvalidOperationException("Expected ERROR_ACCESS_DENIED, got " + error);
            Console.WriteLine("{\"restrictedTokenDenied\":true,\"win32Error\":5}");
        }
        finally
        {
            if (impersonation != IntPtr.Zero) CloseHandle(impersonation);
            if (token != IntPtr.Zero) CloseHandle(token);
            pinned.Free();
        }
    }

    // The owner's DACL permits this open, but SECURITY_ANONYMOUS deliberately
    // withholds an authenticated impersonation identity. The server must refuse it.
    static void ProbeAnonymous(string name)
    {
        using (var handle = CreateFile(name, 0xc0000000, 0, IntPtr.Zero, 3, 0x00100000, IntPtr.Zero))
        {
            if (handle.IsInvalid) throw new Win32Exception(Marshal.GetLastWin32Error());
            using (var pipe = new FileStream(handle, FileAccess.ReadWrite))
            {
                byte[] request = Utf8.GetBytes("{\"expression\":\"must-not-evaluate\"}\n");
                pipe.Write(request, 0, request.Length); pipe.Flush();
                try
                {
                    if (pipe.ReadByte() != -1) throw new InvalidOperationException("Anonymous debug client received a response");
                }
                catch (IOException error)
                {
                    int code = error.HResult & 0xffff;
                    if (code != 109 && code != 232 && code != 233) throw;
                }
            }
        }
        Console.WriteLine("{\"anonymousClientRejected\":true}");
    }

    public static int Main(string[] args)
    {
        Console.OutputEncoding = new UTF8Encoding(false);
        try
        {
            if (args.Length != 2 || !args[1].StartsWith(@"\\.\pipe\gpui-box-debug-", StringComparison.Ordinal))
                throw new ArgumentException("Expected serve|probe-denied|probe-anonymous and a gpui-box debug pipe name");
            if (args[0] == "serve") Serve(args[1]);
            else if (args[0] == "probe-denied") ProbeDenied(args[1]);
            else if (args[0] == "probe-anonymous") ProbeAnonymous(args[1]);
            else throw new ArgumentException("Unknown debug helper command");
            return 0;
        }
        catch (Exception error) { Console.Error.WriteLine("Windows debug transport: " + error.Message); return 1; }
    }
}

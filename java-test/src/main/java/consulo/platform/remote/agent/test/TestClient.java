/*
 * Copyright 2013-2026 consulo.io
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
package consulo.platform.remote.agent.test;

import consulo.platform.remote.agent.protocol.*;
import org.apache.thrift.TException;
import org.apache.thrift.protocol.TBinaryProtocol;
import org.apache.thrift.transport.TSocket;
import org.apache.thrift.transport.TTransport;

import java.nio.charset.Charset;
import java.util.List;

public class TestClient {

    private static final String DEFAULT_HOST = "127.0.0.1";
    private static final int DEFAULT_PORT = 57638;

    public static void main(String[] args) {
        String host = args.length > 0 ? args[0] : DEFAULT_HOST;
        int port = args.length > 1 ? Integer.parseInt(args[1]) : DEFAULT_PORT;

        System.out.println("Connecting to remote-agent at " + host + ":" + port + "...");

        try (TTransport transport = new TSocket(host, port)) {
            transport.open();

            TBinaryProtocol protocol = new TBinaryProtocol(transport);
            RemoteAgentService.Client client = new RemoteAgentService.Client(protocol);

            // --- Agent Identity ---
            testAgentInfo(client);

            // --- Workspace ---
            testWorkspace(client);

            // --- System Info ---
            testSystemInfo(client);

            // --- User Info ---
            testUserInfo(client);

            // --- File Operations ---
            testFileOperations(client);

            // --- Process ---
            testProcess(client);

            System.out.println("\n=== All tests passed ===");
        }
        catch (Exception e) {
            System.err.println("Error: " + e.getMessage());
            e.printStackTrace();
            System.exit(1);
        }
    }

    private static void testAgentInfo(RemoteAgentService.Client client) throws TException {
        System.out.println("\n--- Agent Info ---");
        AgentInfo info = client.getAgentInfo();
        System.out.println("  agentId: " + info.getAgentId());
        System.out.println("  version: " + info.getVersion());
    }

    private static void testWorkspace(RemoteAgentService.Client client) throws TException {
        System.out.println("\n--- Workspace ---");
        String path = client.getWorkspacePath();
        System.out.println("  workspace: " + path);
    }

    private static void testSystemInfo(RemoteAgentService.Client client) throws TException {
        System.out.println("\n--- System Info ---");
        SystemInfo info = client.getSystemInfo();
        System.out.println("  os:       " + info.getOsName() + " " + info.getOsVersion());
        System.out.println("  arch:     " + info.getArch());
        System.out.println("  hostname: " + info.getHostname());
        System.out.println("  cpus:     " + info.getCpuCount());
        System.out.println("  memory:   " + (info.getTotalMemory() / 1024 / 1024) + " MB");
        System.out.println("  encoding: " + info.getConsoleEncoding());
        System.out.println("  locale:   " + info.getLocale());
    }

    private static void testUserInfo(RemoteAgentService.Client client) throws TException {
        System.out.println("\n--- User Info ---");
        UserInfo info = client.getUserInfo();
        System.out.println("  user: " + info.getUserName());
        System.out.println("  home: " + info.getHomePath());
    }

    private static void testFileOperations(RemoteAgentService.Client client) throws TException {
        System.out.println("\n--- File Operations ---");

        // List roots
        List<FileInfo> roots = client.listRoots();
        System.out.println("  roots: " + roots.size());
        for (FileInfo root : roots) {
            System.out.println("    " + root.getPath());
        }

        // List workspace directory
        String workspace = client.getWorkspacePath();
        System.out.println("  listing workspace: " + workspace);
        List<FileInfo> files = client.listDirectory(workspace);
        System.out.println("  entries: " + files.size());
        for (FileInfo f : files) {
            String type = f.isDirectory() ? "DIR " : "FILE";
            System.out.println("    [" + type + "] " + f.getName() + " (" + f.getSize() + " bytes)");
        }

        // Create and delete a test directory
        String testDir = workspace + "/test-dir";
        client.createDirectory(testDir, true);
        System.out.println("  created: " + testDir + " (exists=" + client.fileExists(testDir) + ")");
        client.deleteFile(testDir);
        System.out.println("  deleted: " + testDir + " (exists=" + client.fileExists(testDir) + ")");
    }

    private static void testProcess(RemoteAgentService.Client client) throws TException {
        System.out.println("\n--- Process ---");

        // Detect OS to pick the right command
        SystemInfo sysInfo = client.getSystemInfo();
        boolean isWindows = sysInfo.getOsName().toLowerCase().contains("windows");
        Charset charset = Charset.forName(sysInfo.getConsoleEncoding());

        String command;
        List<String> args;
        if (isWindows) {
            command = "cmd.exe";
            args = List.of("/c", "echo hello from remote-agent");
        }
        else {
            command = "/bin/sh";
            args = List.of("-c", "echo hello from remote-agent");
        }

        ProcessInfo proc = client.startProcess(command, args, "", new java.util.HashMap<>());
        System.out.println("  started pid=" + proc.getPid());

        // Poll for output
        for (int i = 0; i < 20; i++) {
            try {
                Thread.sleep(100);
            }
            catch (InterruptedException ignored) {
            }

            ProcessOutput output = client.readProcessOutput(proc.getPid());

            if (output.getStdoutData() != null && output.getStdoutData().length > 0) {
                String text = new String(output.getStdoutData(), charset);
                System.out.print("  stdout: " + text);
            }
            if (output.getStderrData() != null && output.getStderrData().length > 0) {
                String text = new String(output.getStderrData(), charset);
                System.out.print("  stderr: " + text);
            }

            if (output.isSetExitCode()) {
                System.out.println("  exit code: " + output.getExitCode());
                break;
            }
        }

        // List processes
        List<ProcessInfo> processes = client.listProcesses();
        System.out.println("  tracked processes: " + processes.size());
    }
}

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
 */package consulo.platform.remote.agent.test;

import com.google.protobuf.ByteString;
import consulo.platform.remote.agent.protocol.*;
import io.grpc.ManagedChannel;
import io.grpc.Metadata;
import io.grpc.Status;
import io.grpc.StatusRuntimeException;
import io.grpc.netty.GrpcSslContexts;
import io.grpc.netty.NettyChannelBuilder;
import io.grpc.stub.MetadataUtils;
import io.grpc.stub.StreamObserver;

import javax.net.ssl.SSLEngine;
import javax.net.ssl.X509ExtendedTrustManager;
import java.io.ByteArrayOutputStream;
import java.net.Socket;
import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.security.cert.CertificateException;
import java.security.cert.X509Certificate;
import java.util.HexFormat;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;

/**
 * Smoke test for a running remote agent: every call is checked, the first failure exits with 1.
 * Usage: TestClient [host] [port]; REMOTE_AGENT_FINGERPRINT turns on TLS pinned to that certificate,
 * REMOTE_AGENT_TOKEN sends the agent token.
 */
public class TestClient {
    private static final String DEFAULT_HOST = "127.0.0.1";
    private static final int DEFAULT_PORT = 57638;
    private static final long TIMEOUT_SECONDS = 30;
    private static final Metadata.Key<String> PERMISSION_KEY = Metadata.Key.of("permission", Metadata.ASCII_STRING_MARSHALLER);
    private static final Metadata.Key<String> AUTHORIZATION_KEY = Metadata.Key.of("authorization", Metadata.ASCII_STRING_MARSHALLER);

    private static final Empty EMPTY = Empty.getDefaultInstance();

    private final RemoteAgentServiceGrpc.RemoteAgentServiceBlockingStub myBlocking;
    private final RemoteAgentServiceGrpc.RemoteAgentServiceStub myAsync;

    private boolean myWindows;
    private Charset myCharset = StandardCharsets.UTF_8;
    private String myWorkspace = "";
    private int myChecks;

    private TestClient(ManagedChannel channel) {
        myBlocking = RemoteAgentServiceGrpc.newBlockingStub(channel);
        myAsync = RemoteAgentServiceGrpc.newStub(channel);
    }

    public static void main(String[] args) throws Exception {
        String host = args.length > 0 ? args[0] : DEFAULT_HOST;
        int port = args.length > 1 ? Integer.parseInt(args[1]) : DEFAULT_PORT;
        String token = System.getenv("REMOTE_AGENT_TOKEN");
        String fingerprint = System.getenv("REMOTE_AGENT_FINGERPRINT");
        System.out.println("Connecting to remote-agent at " + host + ":" + port
            + (fingerprint != null ? " over TLS" : "") + (token != null ? " with a token" : "") + "...");

        ManagedChannel channel = channel(host, port, fingerprint, token);
        TestClient client = new TestClient(channel);
        try {
            if (token != null || fingerprint != null) {
                client.testSecurity(host, port, fingerprint, token);
            }
            client.testIdentity();
            client.testEnvironment();
            client.testFiles();
            client.testFileStreams();
            client.testProcesses();
            client.testHttp();
            client.testWebSocket();
            client.testFreePort();
            System.out.println("\n=== All " + client.myChecks + " checks passed ===");
        }
        catch (Throwable e) {
            System.err.println("\nFAILED: " + e);
            e.printStackTrace();
            System.exit(1);
        }
        finally {
            channel.shutdownNow().awaitTermination(5, TimeUnit.SECONDS);
        }
    }

    private static ManagedChannel channel(String host, int port, String fingerprint, String token) throws Exception {
        NettyChannelBuilder builder = NettyChannelBuilder.forAddress(host, port).maxInboundMessageSize(64 * 1024 * 1024);
        if (fingerprint != null) {
            builder.sslContext(GrpcSslContexts.forClient().trustManager(new PinnedTrustManager(fingerprint)).build());
        }
        else {
            builder.usePlaintext();
        }
        if (token != null) {
            Metadata headers = new Metadata();
            headers.put(AUTHORIZATION_KEY, "Bearer " + token);
            builder.intercept(MetadataUtils.newAttachHeadersInterceptor(headers));
        }
        return builder.build();
    }

    // --- Security ---

    private void testSecurity(String host, int port, String fingerprint, String token) throws Exception {
        section("Security");
        if (token != null) {
            expectStatusOn(channel(host, port, fingerprint, null), Status.Code.UNAUTHENTICATED, "a call without the token");
            expectStatusOn(channel(host, port, fingerprint, token + "-wrong"), Status.Code.UNAUTHENTICATED, "a call with a wrong token");
        }
        if (fingerprint != null) {
            String wrong = "00" + fingerprint.substring(2);
            expectStatusOn(channel(host, port, wrong, token), Status.Code.UNAVAILABLE, "a server with another certificate");
        }
        check(!myBlocking.getAgentInfo(EMPTY).getAgentId().isEmpty(), "the configured channel is accepted");
    }

    private void expectStatusOn(ManagedChannel channel, Status.Code expected, String what) throws Exception {
        try {
            expectStatus(expected, what, () -> RemoteAgentServiceGrpc.newBlockingStub(channel).getAgentInfo(EMPTY));
        }
        finally {
            channel.shutdownNow().awaitTermination(5, TimeUnit.SECONDS);
        }
    }

    /** Trusts exactly the certificate with the given SHA-256 fingerprint; the host name is not checked. */
    private static final class PinnedTrustManager extends X509ExtendedTrustManager {
        private final String myFingerprint;

        PinnedTrustManager(String fingerprint) {
            myFingerprint = normalize(fingerprint);
        }

        private static String normalize(String fingerprint) {
            return fingerprint.replace(":", "").trim().toUpperCase(java.util.Locale.ROOT);
        }

        private void check(X509Certificate[] chain) throws CertificateException {
            if (chain == null || chain.length == 0) {
                throw new CertificateException("No server certificate");
            }
            try {
                byte[] digest = MessageDigest.getInstance("SHA-256").digest(chain[0].getEncoded());
                if (!HexFormat.of().withUpperCase().formatHex(digest).equals(myFingerprint)) {
                    throw new CertificateException("Server certificate fingerprint does not match");
                }
            }
            catch (NoSuchAlgorithmException e) {
                throw new CertificateException(e);
            }
        }

        @Override
        public void checkServerTrusted(X509Certificate[] chain, String authType) throws CertificateException {
            check(chain);
        }

        @Override
        public void checkServerTrusted(X509Certificate[] chain, String authType, Socket socket) throws CertificateException {
            check(chain);
        }

        @Override
        public void checkServerTrusted(X509Certificate[] chain, String authType, SSLEngine engine) throws CertificateException {
            check(chain);
        }

        @Override
        public void checkClientTrusted(X509Certificate[] chain, String authType) throws CertificateException {
            throw new CertificateException("Not a server");
        }

        @Override
        public void checkClientTrusted(X509Certificate[] chain, String authType, Socket socket) throws CertificateException {
            throw new CertificateException("Not a server");
        }

        @Override
        public void checkClientTrusted(X509Certificate[] chain, String authType, SSLEngine engine) throws CertificateException {
            throw new CertificateException("Not a server");
        }

        @Override
        public X509Certificate[] getAcceptedIssuers() {
            return new X509Certificate[0];
        }
    }

    // --- Identity / system ---

    private void testIdentity() {
        section("Identity");
        AgentInfo info = myBlocking.getAgentInfo(EMPTY);
        check(!info.getAgentId().isEmpty(), "agent id: " + info.getAgentId());
        check(!info.getVersion().isEmpty(), "version: " + info.getVersion());
        check(info.getPermissionsCount() > 0, "permissions: " + info.getPermissionsList());

        myWorkspace = myBlocking.getWorkspacePath(EMPTY).getPath();
        check(!myWorkspace.isEmpty(), "workspace: " + myWorkspace);

        SystemInfo system = myBlocking.getSystemInfo(EMPTY);
        myWindows = system.getOsName().toLowerCase().contains("windows");
        myCharset = Charset.forName(system.getConsoleEncoding());
        check(!system.getArch().isEmpty(), "system: " + system.getOsName() + " " + system.getOsVersion() + " " + system.getArch()
            + ", host " + system.getHostname() + ", " + system.getConsoleEncoding() + ", " + system.getLocale());

        StatInfo stat = myBlocking.getStat(EMPTY);
        check(stat.getCpuCount() > 0 && stat.getTotalMemory() > 0, "stat: " + stat.getCpuCount() + " cpus, load "
            + String.format("%.2f", stat.getCpuLoad()) + ", " + (stat.getUsedMemory() >> 20) + "/" + (stat.getTotalMemory() >> 20) + " MB");

        UserInfo user = myBlocking.getUserInfo(EMPTY);
        check(!user.getUserName().isEmpty() && !"unknown".equals(user.getUserName()), "user: " + user.getUserName() + ", home " + user.getHomePath());
    }

    private void testEnvironment() {
        section("Environment");
        EnvVariables variables = myBlocking.getEnvVariables(EMPTY);
        check(variables.containsVariables("PATH") || variables.containsVariables("Path"), "variables: " + variables.getVariablesCount());

        EnvVariable path = myBlocking.getEnvVariable(EnvVariableRequest.newBuilder().setName(myWindows ? "Path" : "PATH").build());
        check(path.hasValue(), "PATH is set");

        EnvVariable missing = myBlocking.getEnvVariable(EnvVariableRequest.newBuilder().setName("CONSULO_REMOTE_AGENT_MISSING").build());
        check(!missing.hasValue(), "a missing variable has no value");
    }

    // --- Files ---

    private void testFiles() {
        section("Files");
        FileInfoList roots = myBlocking.listRoots(EMPTY);
        check(roots.getFilesCount() > 0, "roots: " + roots.getFilesList().stream().map(FileInfo::getPath).toList());

        String directory = myWorkspace + "/test-client/nested";
        myBlocking.createDirectory(CreateDirectoryRequest.newBuilder().setPath(directory).setRecursive(true).build());
        check(exists(directory), "recursive createDirectory");

        expectStatus(Status.Code.ALREADY_EXISTS, "createDirectory of an existing directory",
            () -> myBlocking.createDirectory(CreateDirectoryRequest.newBuilder().setPath(directory).build()));

        String file = directory + "/hello.txt";
        byte[] content = "hello from the test client\n".getBytes(StandardCharsets.UTF_8);
        myBlocking.writeFile(WriteFileRequest.newBuilder().setPath(file).setData(ByteString.copyFrom(content)).build());
        byte[] read = myBlocking.readFile(path(file)).getData().toByteArray();
        check(java.util.Arrays.equals(content, read), "writeFile + readFile round trip (" + read.length + " bytes)");

        List<FileInfo> entries = myBlocking.listDirectory(path(directory)).getFilesList();
        FileInfo entry = entries.stream().filter(it -> it.getName().equals("hello.txt")).findFirst().orElse(null);
        check(entry != null && !entry.getDirectory() && entry.getSize() == content.length && entry.hasLastModified(),
            "listDirectory sees the file with size and timestamp");

        if (!myWindows) {
            boolean changed = myBlocking.setPermissions(SetPermissionsRequest.newBuilder().setPath(file).setMode(0700).build()).getChanged();
            FileInfo executable = myBlocking.listDirectory(path(directory)).getFilesList().stream()
                .filter(it -> it.getName().equals("hello.txt")).findFirst().orElseThrow();
            check(changed && executable.getExecutable(), "setPermissions 0700 makes the file executable");
        }

        expectStatus(Status.Code.NOT_FOUND, "readFile of a missing file", () -> myBlocking.readFile(path(directory + "/missing.txt")));

        check(myBlocking.deleteFile(path(myWorkspace + "/test-client")).getDeleted(), "deleteFile removes the tree");
        check(!exists(directory), "the tree is gone");
        check(!myBlocking.deleteFile(path(myWorkspace + "/test-client")).getDeleted(), "deleteFile of a missing path answers false");
    }

    private void testFileStreams() throws Exception {
        section("File streams");
        String file = myWorkspace + "/test-client-stream.bin";
        byte[] content = new byte[3 * 1024 * 1024 + 123];
        new Random(42).nextBytes(content);

        CountDownLatch written = new CountDownLatch(1);
        Throwable[] writeError = new Throwable[1];
        StreamObserver<WriteStreamInput> upload = myAsync.writeFileStream(new StreamObserver<>() {
            @Override
            public void onNext(Empty value) {
            }

            @Override
            public void onError(Throwable error) {
                writeError[0] = error;
                written.countDown();
            }

            @Override
            public void onCompleted() {
                written.countDown();
            }
        });
        upload.onNext(WriteStreamInput.newBuilder().setPath(file).build());
        int chunk = 64 * 1024;
        for (int offset = 0; offset < content.length; offset += chunk) {
            upload.onNext(WriteStreamInput.newBuilder().setData(ByteString.copyFrom(content, offset, Math.min(chunk, content.length - offset))).build());
        }
        upload.onCompleted();
        check(written.await(TIMEOUT_SECONDS, TimeUnit.SECONDS) && writeError[0] == null, "writeFileStream of " + content.length + " bytes");

        ByteArrayOutputStream download = new ByteArrayOutputStream();
        int chunks = 0;
        Iterator<FileContent> stream = myBlocking.readFileStream(ReadStreamRequest.newBuilder().setPath(file).setChunkSize(256 * 1024).build());
        while (stream.hasNext()) {
            stream.next().getData().writeTo(download);
            chunks++;
        }
        check(java.util.Arrays.equals(content, download.toByteArray()), "readFileStream returns the same bytes in " + chunks + " chunks");

        check(myBlocking.deleteFile(path(file)).getDeleted(), "cleanup");
    }

    // --- Processes ---

    private void testProcesses() throws Exception {
        section("Processes");
        ExecResult echo = exec(shell("echo hello from remote-agent"), List.of());
        check(echo.exitCode() == 0 && echo.stdout().trim().equals("hello from remote-agent"), "echo: " + echo.stdout().trim());

        ExecResult stderr = exec(shell(myWindows ? "echo oops 1>&2 & exit 3" : "echo oops >&2; exit 3"), List.of());
        check(stderr.exitCode() == 3 && stderr.stderr().trim().equals("oops"), "stderr and exit code 3");

        if (!myWindows) {
            ExecResult cat = exec(ExecStart.newBuilder().setCommand("cat").build(), List.of("line one\n", "line two\n"));
            check(cat.exitCode() == 0 && cat.stdout().equals("line one\nline two\n"), "stdin is piped to cat and echoed back");

            ExecResult env = exec(shell("echo $CONSULO_TEST_VALUE").toBuilder().putEnvironment("CONSULO_TEST_VALUE", "forty-two").build(), List.of());
            check(env.stdout().trim().equals("forty-two"), "environment is passed");

            ExecResult cwd = exec(shell("pwd").toBuilder().setWorkingDirectory(myWorkspace).build(), List.of());
            check(cwd.stdout().trim().equals(myWorkspace), "working directory: " + cwd.stdout().trim());

            testKill();
        }

        expectStatus(Status.Code.NOT_FOUND, "exec of a missing executable",
            () -> exec(ExecStart.newBuilder().setCommand("/definitely/missing/binary").build(), List.of()));
    }

    private void testKill() throws Exception {
        BlockingQueue<ExecEvent> events = new LinkedBlockingQueue<>();
        StreamObserver<ExecInput> input = myAsync.exec(collect(events));
        input.onNext(ExecInput.newBuilder().setStart(shell("sleep 60")).build());
        long pid = next(events).getStarted();

        List<ProcessInfo> processes = myBlocking.listProcesses(EMPTY).getProcessesList();
        check(processes.stream().anyMatch(it -> it.getPid() == pid && it.getAlive()), "listProcesses sees pid " + pid);

        check(myBlocking.killProcess(KillRequest.newBuilder().setPid(pid).setForce(false).build()).getKilled(), "killProcess by pid (SIGTERM)");
        ExecEvent exit = next(events);
        while (exit.getEventCase() != ExecEvent.EventCase.EXITED) {
            exit = next(events);
        }
        check(!exit.getExited().hasExitCode() && exit.getExited().getSignal() == 15, "the exit reports signal 15");

        BlockingQueue<ExecEvent> forced = new LinkedBlockingQueue<>();
        StreamObserver<ExecInput> forcedInput = myAsync.exec(collect(forced));
        forcedInput.onNext(ExecInput.newBuilder().setStart(shell("sleep 60")).build());
        next(forced);
        forcedInput.onNext(ExecInput.newBuilder().setKill(ExecKill.newBuilder().setForce(true)).build());
        ExecEvent forcedExit = next(forced);
        while (forcedExit.getEventCase() != ExecEvent.EventCase.EXITED) {
            forcedExit = next(forced);
        }
        check(forcedExit.getExited().getSignal() == 9, "kill message on the stream (SIGKILL)");
    }

    private ExecStart shell(String script) {
        return myWindows
            ? ExecStart.newBuilder().setCommand("cmd.exe").addArguments("/c").addArguments(script).build()
            : ExecStart.newBuilder().setCommand("/bin/sh").addArguments("-c").addArguments(script).build();
    }

    private ExecResult exec(ExecStart start, List<String> stdin) throws Exception {
        BlockingQueue<ExecEvent> events = new LinkedBlockingQueue<>();
        StreamObserver<ExecInput> input = myAsync.exec(collect(events));
        input.onNext(ExecInput.newBuilder().setStart(start).build());
        for (String line : stdin) {
            input.onNext(ExecInput.newBuilder().setStdin(ByteString.copyFrom(line, myCharset)).build());
        }
        input.onNext(ExecInput.newBuilder().setCloseStdin(EMPTY).build());

        ByteArrayOutputStream out = new ByteArrayOutputStream();
        ByteArrayOutputStream err = new ByteArrayOutputStream();
        while (true) {
            ExecEvent event = next(events);
            switch (event.getEventCase()) {
                case STDOUT -> event.getStdout().writeTo(out);
                case STDERR -> event.getStderr().writeTo(err);
                case EXITED -> {
                    input.onCompleted();
                    return new ExecResult(event.getExited().getExitCode(), out.toString(myCharset), err.toString(myCharset));
                }
                default -> {
                }
            }
        }
    }

    private record ExecResult(int exitCode, String stdout, String stderr) {
    }

    // --- HTTP / WebSocket ---

    private void testHttp() {
        section("HTTP client");
        HttpResponse get = myBlocking.executeHttpRequest(HttpRequest.newBuilder()
            .setMethod("GET")
            .setUrl("https://postman-echo.com/get?from=consulo")
            .putHeaders("X-Consulo", "test")
            .setConnectTimeoutMillis(10_000)
            .setReadTimeoutMillis(10_000)
            .build());
        String body = get.getBody().toString(StandardCharsets.UTF_8);
        check(get.getStatusCode() == 200 && body.contains("\"from\":\"consulo\"") && body.contains("\"x-consulo\":\"test\""),
            "GET 200 with query and header echoed");

        HttpResponse post = myBlocking.executeHttpRequest(HttpRequest.newBuilder()
            .setMethod("POST")
            .setUrl("https://postman-echo.com/post")
            .putHeaders("Content-Type", "application/json")
            .setBody(ByteString.copyFromUtf8("{\"test\":true}"))
            .build());
        check(post.getStatusCode() == 200 && post.getBody().toStringUtf8().contains("\"test\":true"), "POST body echoed");

        HttpResponse put = myBlocking.executeHttpRequest(HttpRequest.newBuilder().setMethod("PUT").setUrl("https://postman-echo.com/put").build());
        check(put.getStatusCode() == 200, "PUT is supported");

        HttpResponse notFound = myBlocking.executeHttpRequest(HttpRequest.newBuilder().setMethod("GET").setUrl("https://postman-echo.com/status/404").build());
        check(notFound.getStatusCode() == 404 && !notFound.getHeadersMap().isEmpty(), "an error status keeps its headers");

        HttpResponse redirect = myBlocking.executeHttpRequest(HttpRequest.newBuilder()
            .setMethod("GET").setUrl("https://postman-echo.com/redirect-to?url=https%3A%2F%2Fpostman-echo.com%2Fget").setMaxRedirects(0).build());
        check(redirect.getStatusCode() == 302, "maxRedirects 0 returns the redirect itself");
    }

    private void testWebSocket() throws Exception {
        section("WebSocket proxy");
        BlockingQueue<WebSocketEvent> events = new LinkedBlockingQueue<>();
        StreamObserver<WebSocketInput> input = myAsync.webSocket(collect(events));
        input.onNext(WebSocketInput.newBuilder().setConnect(WebSocketConnect.newBuilder().setUrl("wss://echo.websocket.org")).build());
        check(next(events).getEventCase() == WebSocketEvent.EventCase.OPENED, "connected");

        WebSocketEvent greeting = next(events);
        check(greeting.getEventCase() == WebSocketEvent.EventCase.TEXT, "server greeting: " + greeting.getText());

        input.onNext(WebSocketInput.newBuilder().setText("hello websocket").build());
        WebSocketEvent text = next(events);
        check(text.getEventCase() == WebSocketEvent.EventCase.TEXT && text.getText().equals("hello websocket"), "text frame echoed");

        byte[] payload = {1, 2, 3, 4, 5};
        input.onNext(WebSocketInput.newBuilder().setBinary(ByteString.copyFrom(payload)).build());
        WebSocketEvent binary = next(events);
        check(binary.getEventCase() == WebSocketEvent.EventCase.BINARY && binary.getBinary().equals(ByteString.copyFrom(payload)),
            "binary frame echoed");
        input.onCompleted();
    }

    private void testFreePort() {
        section("Utility");
        int port = myBlocking.findFreePort(EMPTY).getPort();
        check(port > 0 && port < 65536, "free port: " + port);
    }

    // --- helpers ---

    private boolean exists(String path) {
        return myBlocking.fileExists(path(path)).getExists();
    }

    private static PathRequest path(String path) {
        return PathRequest.newBuilder().setPath(path).build();
    }

    private static <T> StreamObserver<T> collect(BlockingQueue<T> events) {
        STREAM_ERRORS.remove(events);
        return new StreamObserver<>() {
            @Override
            public void onNext(T value) {
                events.add(value);
            }

            @Override
            public void onError(Throwable error) {
                STREAM_ERRORS.put(events, error);
            }

            @Override
            public void onCompleted() {
            }
        };
    }

    private static final Map<BlockingQueue<?>, Throwable> STREAM_ERRORS = new ConcurrentHashMap<>();

    private static <T> T next(BlockingQueue<T> events) throws Exception {
        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(TIMEOUT_SECONDS);
        while (System.nanoTime() < deadline) {
            T event = events.poll(100, TimeUnit.MILLISECONDS);
            if (event != null) {
                return event;
            }
            Throwable error = STREAM_ERRORS.get(events);
            if (error instanceof StatusRuntimeException statusError) {
                throw statusError;
            }
            if (error != null) {
                throw new AssertionError("stream failed", error);
            }
        }
        throw new AssertionError("no event in " + TIMEOUT_SECONDS + "s");
    }

    private void expectStatus(Status.Code expected, String what, ThrowingRunnable call) {
        try {
            call.run();
            check(false, what + " should fail with " + expected);
        }
        catch (StatusRuntimeException e) {
            String permission = e.getTrailers() == null ? null : e.getTrailers().get(PERMISSION_KEY);
            check(e.getStatus().getCode() == expected, what + " -> " + e.getStatus().getCode()
                + (permission != null ? " [" + permission + "]" : "") + ": " + e.getStatus().getDescription());
        }
        catch (Exception e) {
            throw new AssertionError(what, e);
        }
    }

    @FunctionalInterface
    private interface ThrowingRunnable {
        void run() throws Exception;
    }

    private static void section(String name) {
        System.out.println("\n--- " + name + " ---");
    }

    private void check(boolean condition, String message) {
        myChecks++;
        if (!condition) {
            throw new AssertionError(message);
        }
        System.out.println("  ok  " + message);
    }
}

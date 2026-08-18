package dev.whalefrommars.examples.java;

import dev.whalefrommars.wfobserver.ObserverClient;
import dev.whalefrommars.wfobserver.WFObserver;

public final class Main {
    private Main() {}

    public static void main(String[] args) {
        if (args.length != 1) {
            System.err.println("usage: java-console <endpoint-ticket>");
            System.exit(2);
        }

        try (ObserverClient client = WFObserver.connect(args[0]).join()) {
            try {
                client.ping().join();
            } finally {
                client.shutdown().join();
            }
        }

        System.out.println("WF Observer ping succeeded");
    }
}

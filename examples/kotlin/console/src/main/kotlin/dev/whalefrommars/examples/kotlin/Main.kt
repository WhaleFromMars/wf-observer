package dev.whalefrommars.examples.kotlin

import dev.whalefrommars.wfobserver.WFObserver
import kotlin.system.exitProcess

fun main(args: Array<String>) {
    if (args.size != 1) {
        System.err.println("usage: kotlin-console <endpoint-ticket>")
        exitProcess(2)
    }

    WFObserver.connect(args.single()).join().use { client ->
        try {
            client.ping().join()
        } finally {
            client.shutdown().join()
        }
    }

    println("WF Observer ping succeeded")
}

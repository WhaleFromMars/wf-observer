using WFObserver;

if (args.Length != 1)
{
    Console.Error.WriteLine("usage: dotnet run -- <endpoint-ticket>");
    Environment.ExitCode = 2;
    return;
}

using var client = await Wf_observer_ffi.Connect(args[0]);

try
{
    await client.Ping();
}
finally
{
    await client.Shutdown();
}

Console.WriteLine("WF Observer ping succeeded");

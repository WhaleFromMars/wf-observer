fn main() -> Result<(), std::env::VarError> {
    let target = std::env::var("TARGET")?;
    // We require this build script to re-expose the cargo target triple
    // so that the service knows which to download when perfoming an update.
    // The TARGET env is only available at compile time.
    // The alternative is a bunch of cfgs that need maintaining as and when targets update.
    println!("cargo::rustc-env=WF_OBSERVER_TARGET={target}");

    Ok(())
}

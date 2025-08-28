// main.rs in ctx-cli

// app 모듈은 이제 이 크레이트 내에 있습니다.
mod app;

use app::cli::run_cli;

fn main() -> anyhow::Result<()> {
    // run_cli 함수를 직접 호출합니다.
    run_cli().map_err(|e| anyhow::anyhow!("{}", e.user_friendly_message()))?;
    Ok(())
}
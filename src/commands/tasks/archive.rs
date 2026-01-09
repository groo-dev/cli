use anyhow::Result;
use console::style;

use crate::auth::storage::load_auth_with_password;
use crate::tasks::TasksClient;

pub async fn run(id: String) -> Result<()> {
    let (auth, _) = load_auth_with_password()?;
    let client = TasksClient::new(auth.access_token);

    let task = client.archive_task(&id).await?;

    println!(
        "{} Archived: {}",
        style("□").dim(),
        style(&task.title).dim()
    );

    Ok(())
}

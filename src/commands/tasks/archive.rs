use anyhow::Result;
use console::style;

use crate::auth::provider;
use crate::tasks::TasksClient;

use super::resolve_task_id;

pub async fn run(id: String) -> Result<()> {
    let auth = provider::get_valid_auth().await?;
    let client = TasksClient::new(auth.access_token);

    let task_id = resolve_task_id(&client, &id).await?;
    let task = client.archive_task(&task_id).await?;

    println!(
        "{} Archived: {}",
        style("□").dim(),
        style(&task.title).dim()
    );

    Ok(())
}

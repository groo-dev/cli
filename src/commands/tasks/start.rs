use anyhow::Result;
use console::style;

use crate::auth::storage::load_auth_with_password;
use crate::tasks::TasksClient;

use super::resolve_task_id;

pub async fn run(id: String) -> Result<()> {
    let (auth, _) = load_auth_with_password()?;
    let client = TasksClient::new(auth.access_token);

    let task_id = resolve_task_id(&client, &id).await?;
    let task = client.start_task(&task_id).await?;

    println!(
        "{} Started: {}",
        style("●").yellow(),
        style(&task.title).bold()
    );

    Ok(())
}

use anyhow::Result;
use console::style;

use crate::auth::provider;
use crate::tasks::{CreateCommentRequest, TasksClient};

use super::resolve_task_id;

pub async fn run(id: String, content: String) -> Result<()> {
    let auth = provider::get_valid_auth().await?;
    let client = TasksClient::new(auth.access_token);

    let task_id = resolve_task_id(&client, &id).await?;

    let request = CreateCommentRequest {
        content,
        author: Some("cli".to_string()),
    };

    let comment = client.add_comment(&task_id, request).await?;

    println!(
        "{} Added comment to task {}",
        style("✓").green(),
        style(&task_id).dim()
    );
    println!("  {}", style(&comment.content).dim());

    Ok(())
}

use anyhow::Result;
use console::style;

use crate::auth::storage::load_auth_with_password;
use crate::tasks::{CreateCommentRequest, TasksClient};

pub async fn run(id: String, content: String) -> Result<()> {
    let (auth, _) = load_auth_with_password()?;
    let client = TasksClient::new(auth.access_token);

    let request = CreateCommentRequest {
        content,
        author: Some("cli".to_string()),
    };

    let comment = client.add_comment(&id, request).await?;

    println!(
        "{} Added comment to task {}",
        style("✓").green(),
        style(&id[..8]).dim()
    );
    println!("  {}", style(&comment.content).dim());

    Ok(())
}

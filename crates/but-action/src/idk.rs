use gitbutler_command_context::CommandContext;
// use gitbutler_stack::VirtualBranchesHandle;

use crate::{OpenAiProvider, tool_box::ToolBox};

pub fn ugh_just_figure_it_out(
    ctx: &mut CommandContext,
    openai: &OpenAiProvider,
) -> anyhow::Result<()> {
    let repo = ctx.gix_repo()?;

    let project_status = crate::get_project_status(ctx, &repo)?;

    let tool_box = ToolBox::new(ctx, openai);

    let grouping = crate::grouping::group(openai, &project_status)?;
    let serialized_grouping = serde_json::to_string_pretty(&grouping)
        .map_err(|e| anyhow::anyhow!("Failed to serialize grouping: {}", e))?;

    println!("Grouping:\n\n{}", serialized_grouping);

    let prompt = format!(
        "Commit the following groups to the right branches. Please, be very descriptive in the commit messages:
        
        <grouping>
            {}
        </grouping>",
        serialized_grouping
    );

    let response = tool_box.execute(&prompt)?;

    println!("Response from OpenAI: {}", response);

    Ok(())
}

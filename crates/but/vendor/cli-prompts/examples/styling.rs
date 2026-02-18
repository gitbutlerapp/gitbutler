use std::io::Result;

use cli_prompts::{
    DisplayPrompt,
    prompts::{Confirmation, Input},
    style::{Color, ConfirmationStyle, Formatting, InputStyle, LabelStyle},
};

fn name_validation(input: &str) -> std::result::Result<String, String> {
    if input.len() > 0 {
        Ok(input.to_string())
    } else {
        Err(String::from("Name must not be empty"))
    }
}

fn main() -> Result<()> {
    let label_style = LabelStyle::default()
        .prefix("*")
        .prefix_formatting(Formatting::default().foreground_color(Color::Cyan))
        .prompt_formatting(
            Formatting::default()
                .bold()
                .underline()
                .foreground_color(Color::Magenta),
        );
    let input_formatting = Formatting::default().foreground_color(Color::Cyan);
    let submitted_formatting = Formatting::default().foreground_color(Color::DarkCyan);

    let name = Input::new("Enter your name", name_validation)
        .style(
            InputStyle::default()
                .label_style(label_style.clone())
                .input_formatting(input_formatting.clone())
                .submitted_formatting(submitted_formatting.clone()),
        )
        .display();
    let coffee = Confirmation::new("Do you want a cup of coffee")
        .style(
            ConfirmationStyle::default()
                .label_style(label_style.clone())
                .input_formatting(input_formatting.clone())
                .submitted_formatting(submitted_formatting.clone()),
        )
        .display();

    println!("Name: {:?}", name);
    println!("Coffee: {:?}", coffee);

    Ok(())
}

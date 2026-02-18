use cli_prompts::{
    prompts::{Confirmation, Input, Multiselect, Selection},
    DisplayPrompt,
};

#[derive(Debug)]
enum CarModel {
    Audi,
    BMW,
    Chevrolet,
}

fn car_to_string(car: &CarModel) -> String {
    match car {
        CarModel::Audi => "Audi A3".into(),
        CarModel::BMW => "BMW X5".into(),
        CarModel::Chevrolet => "Chevrolet 11".into(),
    }
}

fn main() {
    let desserts = [
        "Tiramisu",
        "Cheesecake",
        "Brownee",
        "Cookie",
        "Jelly",
        "Chupa-Chups",
        "Pudding",
    ];
    let subjects = [
        "Physics",
        "Math",
        "Polish",
        "English",
        "Sport",
        "Geography",
        "History",
    ];
    let cars = [CarModel::Audi, CarModel::BMW, CarModel::Chevrolet];

    let input_prompt = Input::new("Enter your name", |s| Ok(s.to_string()))
        .default_value("John")
        .help_message("Please provide your real name");
    let confirmation = Confirmation::new("Do you want a cup of coffee?").default_positive(true);
    let dessert_selection = Selection::new("Your favoite dessert", desserts.into_iter());
    let car_selection =
        Selection::new_with_transformation("Your car model", cars.into_iter(), car_to_string);
    let subjects_selection =
        Multiselect::new("What are your favourite subjects", subjects.into_iter());

    let name = input_prompt.display();
    let is_coffee = confirmation.display();
    let dessert = dessert_selection.display();
    let car = car_selection.display();
    let subjects = subjects_selection.display();

    println!("Name: {:?}", name);
    println!("Is coffee: {:?}", is_coffee);
    println!("Dessert: {:?}", dessert);
    println!("Car: {:?}", car);
    println!("Subjects: {:?}", subjects);
}

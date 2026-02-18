pub mod multiselect;
pub mod selection;
pub mod multioption_prompt;

/// A helper struct for the multi-option prompts
pub struct Options<T> {
    all_options: Vec<T>,
    transformed_options: Vec<String>,
    filtered_options: Vec<usize>,
}

impl<T> Options<T>
where
    T: Into<String> + Clone,
{
    /// Create `Options` from an iterator over a type that is convertable to `String`
    pub fn from_iter<I>(iter: I) -> Self
    where
        I: Iterator<Item = T>,
    {
        let options: Vec<T> = iter.collect();
        let options_count = options.len();
        Options {
            all_options: options.clone(),
            transformed_options: options.into_iter().map(|s| s.into()).collect(),
            filtered_options: (0..options_count).collect(),
        }
    }
}

impl<T> Options<T> {
    /// Create `Options` from an arbitrary type using provided transformation function to `String`
    pub fn from_iter_transformed<I, F>(iter: I, transformation: F) -> Self
    where
        I: Iterator<Item = T>,
        F: Fn(&T) -> String,
    {
        let all_options: Vec<T> = iter.collect();
        let transformed_options: Vec<String> = all_options.iter().map(transformation).collect();
        let options_count = all_options.len();

        Options {
            all_options,
            transformed_options,
            filtered_options: (0..options_count).collect(),
        }
    }

    /// Filter options using provided string slice
    pub fn filter(&mut self, filter: &str) {
        self.filtered_options.clear();
        for (index, option) in self.transformed_options.iter().enumerate() {
            if option.contains(filter) {
                self.filtered_options.push(index);
            }
        }
    }

    /// Retrieve the indices of all options that satisfy the last applied filter
    pub fn filtered_options(&self) -> &[usize] {
        &self.filtered_options
    }

    /// Get a mutable reference to the vector all available options
    pub fn all_options_mut(&mut self) -> &mut Vec<T> {
        &mut self.all_options
    }

    /// Get a reference to all options in their string representation
    pub fn transformed_options(&self) -> &[String] {
        &self.transformed_options
    }
} 

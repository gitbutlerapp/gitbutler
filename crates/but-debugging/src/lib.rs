use std::{path::Path, process::Command};

pub struct LogOptions {
    pub oneline: bool,
    pub graph: bool,
    pub all: bool,
    pub reference: String,
}

impl Default for LogOptions {
    fn default() -> Self {
        Self {
            oneline: true,
            graph: true,
            all: false,
            reference: "HEAD".to_string(),
        }
    }
}

impl LogOptions {
    pub fn oneline(&mut self, oneline: bool) -> &mut Self {
        self.oneline = oneline;
        self
    }

    pub fn graph(&mut self, graph: bool) -> &mut Self {
        self.graph = graph;
        self
    }

    pub fn reference(&mut self, reference: String) -> &mut Self {
        self.reference = reference;
        self
    }

    pub fn all(&mut self, all: bool) -> &mut Self {
        self.all = all;
        self
    }
}

pub fn git_log(path: &Path, options: &LogOptions) {
    let path = if path.ends_with(".git") {
        path.parent().unwrap()
    } else {
        path
    };
    let mut command = Command::new("git");
    command.current_dir(path);
    command.arg("log");
    if options.oneline {
        command.arg("--oneline");
    }
    if options.graph {
        command.arg("--graph");
    }
    if options.all {
        command.arg("--all");
    }
    command.arg(options.reference.clone());
    let result = command.output().unwrap().stdout;
    println!("{:?}", command);
    println!("{}", std::str::from_utf8(&result).unwrap());
}

pub struct LsTreeOptions {
    pub recursive: bool,
    pub reference: String,
}

impl Default for LsTreeOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            reference: "HEAD".to_string(),
        }
    }
}

impl LsTreeOptions {
    pub fn recursive(&mut self, recursive: bool) -> &mut Self {
        self.recursive = recursive;
        self
    }

    pub fn reference(&mut self, reference: String) -> &mut Self {
        self.reference = reference;
        self
    }
}

pub fn git_ls_tree(path: &Path, options: &LsTreeOptions) {
    let path = if path.ends_with(".git") {
        path.parent().unwrap()
    } else {
        path
    };
    let mut command = Command::new("git");
    command.current_dir(path);
    command.arg("ls-tree");
    if options.recursive {
        command.arg("-r");
    }
    command.arg(options.reference.clone());
    let result = command.output().unwrap().stdout;
    println!("{:?}", command);
    println!("{}", std::str::from_utf8(&result).unwrap());
}

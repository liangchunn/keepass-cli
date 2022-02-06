use clap::Parser;
use console::{style, Term};
use dialoguer::{theme::ColorfulTheme, Password, Select};
use keepass::{Database, Entry, Group, Node, NodeRef, Result};
use std::fs::File;

/// KeePass CLI
#[derive(Parser, Debug)]
struct Args {
    /// Database file path
    db: String,

    /// Searches an entry that matches the title that is given
    #[clap(requires = "password")]
    entry_title: Option<String>,

    /// Path to keyfile
    #[clap(short = 'k', long)]
    keyfile: Option<String>,

    /// Password
    #[clap(short, long)]
    password: Option<String>,
}

struct Selection<'a> {
    kind: NodeRef<'a>,
}

impl<'a> std::fmt::Display for Selection<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self.kind {
                NodeRef::Group(g) => format!("📁 {}", g.name),
                NodeRef::Entry(e) => format!("🔑 {}", e.get_title().unwrap()),
            }
        )
    }
}

#[derive(Debug)]
struct Context<'a> {
    node: &'a Group,
    index: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let term = Term::stderr();

    // TODO: handle no-password databases
    let password = if let Some(password) = args.password {
        password
    } else {
        Password::with_theme(&ColorfulTheme::default())
            .with_prompt("Password")
            .allow_empty_password(true)
            .interact_on(&term)
            .unwrap()
    };

    // Open KeePass database
    let path = std::path::Path::new(&args.db);
    // TODO: trigger the reprompt of password if it's entered incorrectly
    // TODO: if password is passed in by args, we should panic
    let db = Database::open(&mut File::open(path)?, Some(&password), None)?;

    // if we have some entry_title, then we want to only print and don't prompt anything
    if let Some(entry_title) = args.entry_title {
        let search_result = search_entry_by_title(&entry_title, &db.root);
        if search_result.len() == 0 {
            println!("No entries found");
        } else {
            println!(
                "Found {} result(s) for title name \"{}\"",
                search_result.len(),
                entry_title
            );
            for entry in search_result {
                print_entry(entry);
                println!();
            }
        }
    } else {
        let root_context = Context {
            node: &db.root,
            index: 0,
        };

        let mut context: Vec<Context> = vec![root_context];

        prompt(&term, &db.root, &mut context);
    }
    Ok(())
}

fn prompt<'a>(term: &Term, node: &'a Group, context: &'a mut Vec<Context<'a>>) {
    let selections = node
        .children
        .iter()
        .map(|child| match child {
            Node::Group(group) => Selection {
                kind: NodeRef::Group(group),
            },
            Node::Entry(entry) => Selection {
                kind: NodeRef::Entry(entry),
            },
        })
        .collect::<Vec<Selection>>();

    let prompt_message = context
        .iter_mut()
        .map(|group| group.node.name.clone())
        .collect::<Vec<String>>()
        .join(" > ");

    let hint = if context.len() == 1 {
        "(press ESC to exit)"
    } else {
        "(press ESC to go back)"
    };

    let styled_hint = style(hint).dim();

    let last_selected_index = match context.last() {
        Some(context) => context.index,
        None => 0,
    };

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("{} {}", prompt_message, styled_hint))
        .default(last_selected_index)
        .items(&selections[..])
        .interact_on_opt(&Term::stdout())
        .unwrap();

    // this clears the last line, so that we can use context to print out
    // a prompt with the levels "Group > Subgroup" and reduce noise in the output
    let _ = term
        .clear_last_lines(1)
        .expect("unable to clear last line!");

    match selection {
        Some(selected) => {
            // sets the last selected index so that when we go back one level up
            // it will select the previously selected option
            context.last_mut().unwrap().index = selected;
            let selected = &selections[selected];
            match selected.kind {
                NodeRef::Group(g) => {
                    // if we select a group, then we want to push the selection context,
                    // and trigger another prompt to the user
                    context.push(Context { node: g, index: 0 });
                    prompt(term, g, context)
                }
                NodeRef::Entry(e) => {
                    print_entry(e);
                    println!();
                    prompt(term, context.last().unwrap().node, context)
                }
            }
        }
        None => {
            // if the user doesn't select anything, then we go back up one level
            // popping the last context, and then prompt the user again
            // with the last context
            let _ = context.pop();
            if let Some(prev_group) = context.last() {
                prompt(term, prev_group.node, context)
            } else {
                println!();
                println!("END")
            }
        }
    };
}

fn print_entry(entry: &Entry) {
    println!("{}", style(entry.get_title().unwrap()).italic());
    println!("  👤: {}", style(entry.get_username().unwrap()).bold());
    println!("  🔑: {}", style(entry.get_password().unwrap()).bold());
    let notes = entry.get("Notes");
    if let Some(note) = notes {
        if note.len() > 0 {
            println!("  📝: {}", note);
        }
    }
}

fn search_entry_by_title<'a>(title: &'a str, root_node: &'a Group) -> Vec<&'a Entry> {
    let mut result: Vec<&Entry> = vec![];
    for node in root_node {
        match node {
            NodeRef::Entry(e) => {
                if e.get_title().unwrap() == title {
                    result.push(e)
                }
            }
            _ => {}
        }
    }
    return result;
}

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "orrin", version, about = "Owns your projects: files, views, landing, builds and sync")]
pub struct Cli {
    #[arg(long, global = true)]
    pub home: Option<PathBuf>,
    #[arg(long, global = true, help = "Print the raw JSON reply")]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Serve {
        #[arg(long)]
        port: Option<u16>,
    },
    #[command(about = "Send any API request with a JSON body")]
    Call {
        request: String,
        #[arg(default_value = "{}")]
        body: String,
    },
    Ping,
    Status,
    Projects,
    Import {
        folder: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    Views {
        project: Option<String>,
    },
    #[command(subcommand)]
    View(ViewCommand),
    Land {
        view: String,
        #[arg(short = 'm', long, default_value = "")]
        intent: String,
    },
    Undo {
        project: Option<String>,
        #[arg(long)]
        change: Option<String>,
        #[arg(long)]
        author: Option<String>,
    },
    Find(Scoped),
    Search {
        #[command(flatten)]
        scoped: Scoped,
        #[arg(long)]
        regex: bool,
        #[arg(short = 's', long)]
        case_sensitive: bool,
        #[arg(long)]
        paths: Option<String>,
    },
    Outline(Scoped),
    Where {
        tool: String,
    },
    Build(JobArgs),
    Run(JobArgs),
    Test(JobArgs),
    Shot(JobArgs),
    Export(JobArgs),
    Jobs {
        project: Option<String>,
    },
    Logs {
        job: String,
        #[arg(short = 'f', long)]
        follow: bool,
    },
    Pcs,
    Pair,
    Redeem {
        code: String,
        #[arg(long, default_value = "")]
        address: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ViewCommand {
    New {
        project: Option<String>,
        #[arg(long, default_value = "cli")]
        owner: String,
        #[arg(long)]
        claim: Vec<String>,
    },
}

#[derive(Args, Debug)]
pub struct Scoped {
    #[arg(value_name = "[PROJECT] TARGET", num_args = 1..=2, required = true)]
    pub words: Vec<String>,
    #[arg(long)]
    pub view: Option<String>,
    #[arg(long)]
    pub limit: Option<u32>,
}

impl Scoped {
    pub fn split(&self) -> (Option<&str>, &str) {
        match self.words.as_slice() {
            [project, target] => (Some(project), target),
            [target] => (None, target),
            _ => unreachable!("clap allows one or two words"),
        }
    }
}

#[derive(Args, Debug)]
pub struct JobArgs {
    #[command(flatten)]
    pub scoped: Scoped,
    #[arg(long)]
    pub pc: Option<String>,
    #[arg(short = 'f', long)]
    pub follow: bool,
    #[arg(last = true)]
    pub args: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Command {
        Cli::try_parse_from(line.split_whitespace()).unwrap().command
    }

    #[test]
    fn project_is_optional_before_the_target() {
        let Command::Find(scoped) = parse("orrin find game player") else { panic!() };
        assert_eq!(scoped.split(), (Some("game"), "player"));
        let Command::Find(scoped) = parse("orrin find player") else { panic!() };
        assert_eq!(scoped.split(), (None, "player"));
    }

    #[test]
    fn job_commands_take_view_pc_follow_and_args() {
        let Command::Shot(job) = parse("orrin shot game web --view v1 --pc laptop -f -- --scene main") else {
            panic!()
        };
        assert_eq!(job.scoped.split(), (Some("game"), "web"));
        assert_eq!(job.scoped.view.as_deref(), Some("v1"));
        assert_eq!(job.pc.as_deref(), Some("laptop"));
        assert!(job.follow);
        assert_eq!(job.args, ["--scene", "main"]);
    }

    #[test]
    fn json_flag_works_after_the_command() {
        let cli = Cli::try_parse_from(["orrin", "projects", "--json"]).unwrap();
        assert!(cli.json);
        assert!(Cli::try_parse_from(["orrin", "find", "a", "b", "c"]).is_err());
    }
}

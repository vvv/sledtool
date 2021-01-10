use std::{io::Write, path::PathBuf};

/// CLI tool to work with Sled databases
#[derive(argh::FromArgs)]
struct Opts {
    #[argh(positional)]
    dbpath: PathBuf,

    #[argh(subcommand)]
    cmd: Cmd,
}

/// Outout entire content of the database to JSON with hex-encoded buffers
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "export")]
struct Export {}

/// Import entire content of the database from JSON with hex-encoded buffers
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "import")]
struct Import {}

/// Get value of specific key from the database
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "get")]
struct Get {
    #[argh(positional)]
    key: String,

    /// tree to use
    #[argh(option, short = 't')]
    tree: Option<String>,

    /// inhibit hex-encoding the value
    #[argh(switch, short = 'r')]
    raw_value: bool,

    /// inhibit hex-decoding or hex-encoding the key
    #[argh(switch, short = 'R')]
    raw_key: bool,

    /// inhibit hex-decoding the tree name
    #[argh(switch, short = 'T')]
    raw_tree_name: bool,

    /// use `get_gt` instead of `get`
    #[argh(switch, short = 'g')]
    gt: bool,

    /// use `get_lt` instead of `get`
    #[argh(switch, short = 'l')]
    lt: bool,

    /// print key in addition to the value, with `=` sign in between
    #[argh(switch, short = 'K')]
    print_key: bool,

    /// do not print `Not found` to console, just set exit code 1
    #[argh(switch, short = 'q')]
    quiet: bool,
}

#[derive(argh::FromArgs)]
#[argh(subcommand)]
enum Cmd {
    Export(Export),
    Import(Import),
    Get(Get),
}

pub mod sledimporter;

fn main() -> anyhow::Result<()> {
    let opts: Opts = argh::from_env();

    let db: sled::Db = sled::open(opts.dbpath)?;

    match opts.cmd {
        Cmd::Export(Export {}) => {
            let so = std::io::stdout();
            let so = so.lock();
            let mut so = std::io::BufWriter::with_capacity(8192, so);

            writeln!(so, "{{")?;
            for (tn, tree_name) in db.tree_names().into_iter().enumerate() {
                if tn > 0 {
                    write!(so, ",")?;
                } else {
                    write!(so, " ")?;
                }
                writeln!(so, r#""{}":{{"#, hex::encode(&tree_name))?;
                let tree = db.open_tree(&tree_name)?;
                for (vn, x) in tree.into_iter().enumerate() {
                    if vn > 0 {
                        write!(so, " ,")?;
                    } else {
                        write!(so, "  ")?;
                    }
                    let (k, v) = x?;
                    writeln!(so, r#""{}": "{}""#, hex::encode(k), hex::encode(v))?;
                }
                writeln!(so, " }}")?;
            }
            writeln!(so, "}}")?;
        }
        Cmd::Import(Import {}) => {
            let si = std::io::stdin();
            let si = si.lock();
            let si = std::io::BufReader::with_capacity(8192, si);
            use serde::de::DeserializeSeed;
            let () = sledimporter::DbDeserializer(&db)
                .deserialize(&mut serde_json::Deserializer::from_reader(si))?;
        }
        Cmd::Get(Get {
            key,
            tree,
            raw_value,
            raw_key,
            raw_tree_name,
            gt,
            lt,
            print_key,
            quiet,
        }) => {
            if lt && gt {
                anyhow::bail!("--gt and --lt options are specified simultaneously");
            }
            let mut t: &sled::Tree = &db;
            let tree_buf ;
            if let Some(tree_name) = tree {
                let tn = if raw_tree_name {
                    tree_name.as_bytes().to_vec()
                } else {
                    hex::decode(tree_name)?
                };
                tree_buf = db.open_tree(tn)?;
                t = &tree_buf;
            }

            let mut k = if raw_key {
                key.as_bytes().to_vec()
            } else {
                hex::decode(key)?
            };

            let v : Option<_>;
            match (lt, gt) {
                (false, false) => v = t.get(&k)?,
                (true, false) => v = t.get_lt(&k)?.map(|(ke,va)| { k = ke.to_vec(); va }),
                (false, true) => v = t.get_gt(&k)?.map(|(ke,va)| { k = ke.to_vec(); va }),
                (true, true) => unreachable!(),
            }

            if let Some(v) = v {
                if print_key {
                    if raw_key {
                        print!("{}=", String::from_utf8_lossy(&k));
                    } else {
                        print!("{}=", hex::encode(k));
                    }
                }
                if raw_value {
                    println!("{}", String::from_utf8_lossy(&v));
                } else {
                    println!("{}", hex::encode(v));
                }
            } else {
                if !quiet {
                    eprintln!("Not found");
                }
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

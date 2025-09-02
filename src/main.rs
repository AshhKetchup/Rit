use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rit", version = "0.1", about = "A mini git implementation")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new, empty repository
    Init,
    /// Compute object ID and optionally create a blob from a file
    HashObject {
        /// Write the object into the object database
        #[arg(short, long)]
        write: bool,
        /// File to hash
        file: PathBuf,
    },
    /// Provide content or type of repository objects
    CatFile {
        /// Pretty-print the contents of objects
        #[arg(short = 'p')]
        pretty_print: bool,
        /// The object ID (SHA-1 hash)
        oid: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => init(),
        Commands::HashObject { write, file } => hash_object(write, file),
        Commands::CatFile { pretty_print, oid } => cat_file(pretty_print, oid),
    }
}

fn init() {
    fs::create_dir(".git").unwrap();
    fs::create_dir(".git/objects").unwrap();
    fs::create_dir(".git/refs").unwrap();
    fs::write(".git/HEAD", "ref: refs/heads/main\n").unwrap();
    println!("Initialized rit directory");
}

fn hash_object(write: bool, file: PathBuf) {
    let data = fs::read(&file).unwrap();
    let header = format!("blob {}\0", data.len());
    let mut store = Vec::new();
    store.extend_from_slice(header.as_bytes());
    store.extend_from_slice(&data);

    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(&store);
    let oid = hasher.finalize();
    let hex = format!("{:x}", oid);

    if write {
        let dir = format!(".git/objects/{}", &hex[..2]);
        let file_path = format!("{}/{}", dir, &hex[2..]);
        fs::create_dir_all(&dir).unwrap();
        fs::write(file_path, store).unwrap();
    }

    println!("{}", hex);
}

fn cat_file(pretty_print: bool, oid: String) {
    let dir = format!(".git/objects/{}", &oid[..2]);
    let file_path = format!("{}/{}", dir, &oid[2..]);
    let data = fs::read(file_path).unwrap();

    // Split header and content
    let null_pos = data.iter().position(|&b| b == 0).unwrap();
    let header = String::from_utf8_lossy(&data[..null_pos]);
    let content = &data[null_pos + 1..];

    if pretty_print {
        println!("{}", String::from_utf8_lossy(content));
    } else {
        println!("{}", header);
    }
}

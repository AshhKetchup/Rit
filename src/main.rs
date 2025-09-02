use std::error::Error;
use clap::{Parser, Subcommand};
use flate2::bufread::ZlibDecoder;
use std::fs;
use std::io::{self, Write, Read};
use std::path::{Path, PathBuf};
use sha1::{Digest, Sha1};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use anyhow;
use anyhow::{Result};

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
    LsTree {
        #[clap(long)]
        name_only: bool,

        tree_hash: String,
    },
    WriteTree{
        path: Option<PathBuf>,
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => init(),
        Commands::HashObject { write, file } => hash_object(write, file),
        Commands::CatFile { pretty_print, oid } => cat_file(pretty_print, &oid),
        Commands::LsTree { name_only, tree_hash } => {
         ls_tree(name_only, &tree_hash)
        },
        Commands::WriteTree { path } => {
            let path = path.as_deref().unwrap_or(Path::new("."));
            // Now path is a &Path, defaulting to current directory
            match write_tree(Some(path)){
                Ok(hash) => println!("{}", hash),
                Err(E) => println!("Error: {}", E)
            }
        }
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

    // Git's blob header format: "blob <size>\0"
    let header = format!("blob {}\0", data.len());

    // Concatenate header + file data
    let mut store = Vec::new();
    store.extend_from_slice(header.as_bytes());
    store.extend_from_slice(&data);

    // Compute SHA-1 over the uncompressed data
    let mut hasher = Sha1::new();
    hasher.update(&store);
    let oid = hasher.finalize();
    let hex = format!("{:x}", oid);

    if write {
        // Compress before writing
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&store).unwrap();
        let compressed = encoder.finish().unwrap();

        // Write into .rit/objects/xx/yyyy...
        let dir = format!(".git/objects/{}", &hex[..2]);
        let file_path = format!("{}/{}", dir, &hex[2..]);
        fs::create_dir_all(&dir).unwrap();
        fs::write(file_path, compressed).unwrap();
    }

    println!("{}", hex);
}


fn cat_file(pretty_print: bool, sha: &str) {
    let path = format!(".git/objects/{}/{}", &sha[..2], &sha[2..]);
    let compressed = fs::read(path).unwrap();

    let mut decoder = ZlibDecoder::new(&compressed[..]);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).unwrap();

    // Find the first null byte (\0)
    if let Some(null_pos) = decompressed.iter().position(|&b| b == 0) {
        let header = &decompressed[..null_pos]; // e.g. "blob 2122"
        let content = &decompressed[null_pos + 1..];

        let header_str = String::from_utf8_lossy(header);

        // Split header into type + size
        let mut parts = header_str.split_whitespace();
        let obj_type = parts.next().unwrap_or("");
        let _size = parts.next().unwrap_or("");

        if obj_type == "blob" {
            if pretty_print {
                // Pretty print → just print the blob content as is
                print!("{}", String::from_utf8_lossy(content));
            } else {
                // Normal mode → print header + raw content
                println!("{}", header_str);
                println!("{}", String::from_utf8_lossy(content));
            }
        } else {
            eprintln!("Unsupported object type: {}", obj_type);
        }
    }
}

fn ls_tree(name_only: bool, tree_hash: &str) {
    let path = format!(".git/objects/{}/{}", &tree_hash[..2], &tree_hash[2..]);
    let compressed = fs::read(path).unwrap();

    let mut decoder = ZlibDecoder::new(&compressed[..]);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).unwrap();

    // Strip off "tree <size>\0"
    let null_pos = decompressed.iter().position(|&b| b == 0).unwrap();
    let mut entries = &decompressed[null_pos + 1..];

    while !entries.is_empty() {
        // mode + filename until \0
        let null_pos = entries.iter().position(|&b| b == 0).unwrap();
        let header = &entries[..null_pos]; // "100644 filename.txt"

        let mut parts = header.splitn(2, |&b| b == b' ');
        let mode = parts.next().unwrap();
        let filename = parts.next().unwrap();

        // SHA1 → 20 raw bytes after \0
        let sha_start = null_pos + 1;
        let sha_end = sha_start + 20;
        let sha_bytes = &entries[sha_start..sha_end];
        let sha = hex::encode(sha_bytes);

        if name_only {
            println!("{}", String::from_utf8_lossy(filename));
        } else {
            println!(
                "{} {} {}",
                String::from_utf8_lossy(mode),
                sha,
                String::from_utf8_lossy(filename)
            );
        }

        // Move to next entry
        entries = &entries[sha_end..];
    }
}

fn write_tree(path: Option<&Path>)  -> Result<String, Box<dyn Error>> {
    let path = path.unwrap_or_else(|| Path::new(".")); // default to current dir

    let mut entries = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if name == ".git" {
            continue;
        }

        if path.is_dir() {
            // recursive write_tree for subdir
            let hash = write_tree(Some(&path))?;
            let mode = "40000"; // tree mode
            let entry = format!("{mode} {name}\0");
            entries.extend_from_slice(entry.as_bytes());
            entries.extend_from_slice(&hex_to_raw(&hash));
        } else if path.is_file() {
            // hash file contents like `git hash-object -w`
            let data = fs::read(&path)?;
            let object = format!("blob {}\0", data.len());
            let mut store = Vec::new();
            store.extend_from_slice(object.as_bytes());
            store.extend_from_slice(&data);

            let mut hasher = Sha1::new();
            hasher.update(&store);
            let hash = hasher.finalize();
            let hex = hex::encode(&hash);

            // write blob object
            write_object(&hex, &store)?;

            let mode = "100644"; // regular file
            let entry = format!("{mode} {name}\0");
            entries.extend_from_slice(entry.as_bytes());
            entries.extend_from_slice(&hash[..]);
        }
    }

    // Now build the tree object
    let mut header = format!("tree {}\0", entries.len()).into_bytes();
    header.extend_from_slice(&entries);

    let mut hasher = Sha1::new();
    hasher.update(&header);
    let tree_hash = hasher.finalize();
    let hex = hex::encode(&tree_hash);

    write_object(&hex, &header)?;

    //println!("{}", hex);
    Ok(hex)
}

fn write_object(hash: &str, data: &[u8]) -> io::Result<()> {
    let dir = format!(".git/objects/{}", &hash[..2]);
    let file = format!("{}/{}", dir, &hash[2..]);

    fs::create_dir_all(&dir)?;

    if !Path::new(&file).exists() {
        let f = fs::File::create(&file)?;
        let mut encoder = ZlibEncoder::new(f, Compression::default());
        encoder.write_all(data)?;
        encoder.finish()?;
    }

    Ok(())
}

// Convert hex string to raw 20-byte SHA-1
fn hex_to_raw(hex: &str) -> Vec<u8> {
    hex::decode(hex).expect("Invalid hex")
}

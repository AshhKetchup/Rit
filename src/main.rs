use clap::{Parser, Subcommand};
use flate2::bufread::ZlibDecoder;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use sha1::{Digest, Sha1};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::Write;


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
        Commands::CatFile { pretty_print, oid } => cat_file(pretty_print, &oid)
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

use clap::Parser;

use tetra_core::BitBuffer;
use tetra_saps::tmv::enums::logical_chans::LogicalChannel;

mod entities;
use entities::umac::UmacParser;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "TETRA Raw PDU Decoder",
    long_about = "Decodes a raw bitstring as a PDU for the specified SAP and destination component"
)]
struct Args {
    /// Direction: uplink or downlink
    #[arg(help = "Direction: [ ul | dl ]")]
    direction: String,

    /// SAP (Service Access Point) name
    #[arg(help = "SAP name: [ tmv ]")]
    sap: String,

    /// Destination component name
    #[arg(help = "Destination component: [ umac ]")]
    destination: String,

    /// Raw bitstring to decode
    #[arg(help = "Raw bitstring (binary representation) to parse as PDU")]
    bitstring: String,

    #[arg(
        short = 'c',
        long = "channel",
        default_value = "schf",
        help = "Logical channel (for tmv sap): [ schf | schhu | schhd | stch | bnch | bsch | aach ]"
    )]
    channel: String,
}

fn main() {
    eprintln!("[+] TETRA PDU Decoding tool");
    eprintln!("    Wouter Bokslag / Midnight Blue");
    eprintln!(" *  This tool is a MESS and is for testing only  *");
    eprintln!(" *  There be bugs..                              *");

    let args = Args::parse();

    let logical_channel = match args.channel.to_lowercase().as_str() {
        "schf" | "sch_f" | "sch/f" => LogicalChannel::SchF,
        "schhu" | "sch_hu" | "sch/hu" => LogicalChannel::SchHu,
        "schhd" | "sch_hd" | "sch/hd" => LogicalChannel::SchHd,
        "stch" => LogicalChannel::Stch,
        "bnch" => LogicalChannel::Bnch,
        "bsch" => LogicalChannel::Bsch,
        "aach" => LogicalChannel::Aach,
        _ => {
            eprintln!(
                "Error: Unsupported logical channel '{}'. Use: schf, schhu, schhd, stch, bnch, bsch, aach",
                args.channel
            );
            std::process::exit(1);
        }
    };

    let is_downlink = match args.direction.to_lowercase().as_str() {
        "ul" | "uplink" => false,
        "dl" | "downlink" => true,
        _ => {
            eprintln!("Error: Unsupported direction '{}'. Use: ul, dl", args.direction);
            std::process::exit(1);
        }
    };

    match (args.sap.to_lowercase().as_str(), args.destination.to_lowercase().as_str()) {
        ("tmv", "umac") => {
            let pdu = BitBuffer::from_bitstr(args.bitstring.as_str());
            if is_downlink {
                UmacParser::parse_dl(pdu, logical_channel);
            } else {
                UmacParser::parse_ul(pdu, logical_channel);
            }
        }
        _ => {
            eprintln!("Error: Unsupported SAP '{}' or destination '{}'", args.sap, args.destination);
            eprintln!("Supported: tmv umac");
            std::process::exit(1);
        }
    };
}

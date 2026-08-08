//! Prints `bam_core::api::schema::all_schemas()` as JSON to stdout.
//! `frontend/scripts/gen-types.mjs` pipes this into checked-in TypeScript
//! (P9.1) — this binary is the one source of truth the generation reads
//! from, so the frontend can never hand-drift from the Rust types.

fn main() {
    let schemas = bam_core::api::schema::all_schemas();
    println!(
        "{}",
        serde_json::to_string_pretty(&schemas).expect("schemas serialize")
    );
}

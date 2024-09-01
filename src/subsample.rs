// This file contains the code for the autocycler subsample subcommand.

// Copyright 2024 Ryan Wick (rrwick@gmail.com)
// https://github.com/rrwick/Autocycler

// This file is part of Autocycler. Autocycler is free software: you can redistribute it and/or
// modify it under the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later version. Autocycler
// is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
// implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General
// Public License for more details. You should have received a copy of the GNU General Public
// License along with Autocycler. If not, see <http://www.gnu.org/licenses/>.

use rand::{rngs::StdRng, SeedableRng};
use rand::seq::SliceRandom;
use seq_io::fastq::Record;
use std::collections::HashSet;
use std::fs::File;
use std::path::PathBuf;

use crate::log::{section_header, explanation};
use crate::metrics::{ReadSetMetrics, SubsampleMetrics};
use crate::misc::{check_if_dir_is_not_dir, create_dir, fastq_reader, format_float, quit_with_error,
                  spinner};


pub fn subsample(fastq_file: PathBuf, out_dir: PathBuf, genome_size_str: String,
                 subset_count: usize, min_read_depth: f64, seed: u64) {
    let subsample_yaml = out_dir.join("subsample.yaml");
    let genome_size = parse_genome_size(&genome_size_str);
    check_settings(&out_dir, genome_size, subset_count, min_read_depth);
    create_dir(&out_dir);
    starting_message();
    print_settings(&fastq_file, &out_dir, genome_size, subset_count, min_read_depth, seed);

    // TODO: add automatic genome size estimation

    let mut metrics = SubsampleMetrics::new();
    let (input_count, input_bases) = input_fastq_stats(&fastq_file, &mut metrics);
    let reads_per_subset = calculate_subsets(input_count, input_bases, genome_size, min_read_depth);
    save_subsets(&fastq_file, subset_count, input_count, reads_per_subset, &out_dir, seed,
                 &mut metrics);
    metrics.save_to_yaml(&subsample_yaml);
    finished_message();
}


fn check_settings(out_dir: &PathBuf, genome_size: u64, subset_count: usize, min_read_depth: f64) {
    check_if_dir_is_not_dir(out_dir);
    if genome_size < 1 {       quit_with_error("--genome_size must be at least 1"); }
    if subset_count < 1 {      quit_with_error("--count must be at least 2"); }
    if min_read_depth <= 0.0 { quit_with_error("--min_read_depth must be greater than 0"); }
}


fn starting_message() {
    section_header("Starting autocycler subsample");
    explanation("This command subsamples a long-read set into subsets that are maximally \
                 independent from each other.");
}


fn print_settings(fastq_file: &PathBuf, out_dir: &PathBuf, genome_size: u64,
                  subset_count: usize, min_read_depth: f64, seed: u64) {
    eprintln!("Settings:");
    eprintln!("  --reads {}", fastq_file.display());
    eprintln!("  --out_dir {}", out_dir.display());
    eprintln!("  --genome_size {}", genome_size);
    eprintln!("  --count {}", subset_count);
    eprintln!("  --min_read_depth {}", format_float(min_read_depth));
    eprintln!("  --seed {}", seed);
    eprintln!();
}


fn parse_genome_size(genome_size_str: &str) -> u64 {
    let genome_size_str = genome_size_str.trim().to_lowercase();
    if let Ok(size) = genome_size_str.parse::<f64>() {
        return size.round() as u64;
    }
    let multiplier = match genome_size_str.chars().last() {
        Some('k') => 1_000.0,
        Some('m') => 1_000_000.0,
        Some('g') => 1_000_000_000.0,
        _ => { quit_with_error("Error: cannot interpret genome size"); }
    };
    let number_part = &genome_size_str[..genome_size_str.len() - 1];
    if let Ok(size) = number_part.parse::<f64>() {
        return (size * multiplier).round() as u64;
    }
    quit_with_error("Error: cannot interpret genome size");
}


fn input_fastq_stats(fastq_file: &PathBuf, metrics: &mut SubsampleMetrics) -> (usize, u64) {
    let mut read_lengths: Vec<u64> = fastq_reader(fastq_file).records()
        .map(|record| record.expect("Error reading FASTQ file").seq().len() as u64).collect();
    read_lengths.sort_unstable();
    let total_bases = read_lengths.iter().sum();
    let n50_target_bases = total_bases / 2;
    let mut running_total = 0;
    let mut n50 = 0;
    for read_length in &read_lengths {
        running_total += read_length;
        if running_total >= n50_target_bases {
            n50 = *read_length;
            break;
        }
    }
    let total_count = read_lengths.len();
    eprintln!("Input FASTQ:");
    eprintln!("  Read count: {}", total_count);
    eprintln!("  Read bases: {}", total_bases);
    eprintln!("  Read N50 length: {} bp", n50);
    eprintln!();
    metrics.input_reads = ReadSetMetrics { count: total_count, bases: total_bases, n50: n50 };
    (total_count, total_bases)
}


fn calculate_subsets(read_count: usize, read_bases: u64, genome_size: u64, min_depth: f64)
        -> usize {
    section_header("Calculating subset size");
    explanation("Autocycler will now calculate the number of reads to put in each subset.");
    let total_depth = read_bases as f64 / genome_size as f64;
    let mean_read_length = (read_bases as f64 / read_count as f64).round() as u64;
    eprintln!("Total read depth: {:.1}×", total_depth);
    eprintln!("Mean read length: {} bp", mean_read_length);
    eprintln!();
    if total_depth < min_depth {
        quit_with_error("Error: input reads are too shallow to subset");
    }
    eprintln!("Calculating subset sizes:");
    eprintln!("  subset_depth = {} * log_2(4 * total_depth / {}) / 2",
              format_float(min_depth), format_float(min_depth));
    let subset_depth = min_depth * (4.0 * total_depth / min_depth).log2() / 2.0;
    eprintln!("               = {:.1}x", subset_depth);
    let subset_ratio = subset_depth / total_depth;
    let reads_per_subset = (subset_ratio * read_count as f64).round() as usize;
    eprintln!("  reads per subset: {}", reads_per_subset);
    eprintln!();
    reads_per_subset
}


fn save_subsets(input_fastq: &PathBuf, subset_count: usize, input_count: usize,
                reads_per_subset: usize, out_dir: &PathBuf, seed: u64,
                metrics: &mut SubsampleMetrics) {
    section_header("Subsetting reads");
    explanation("The reads are now shuffled and grouped into subset files.");
    let mut rng = StdRng::seed_from_u64(seed);
    let mut read_order: Vec<usize> = (0..input_count).collect();
    read_order.shuffle(&mut rng);
    let mut subset_indices = Vec::new();
    let mut subset_files = Vec::new();
    for i in 0..subset_count {
        eprintln!("subset {}:", i+1);
        subset_indices.push(get_subsample_indices(subset_count, input_count, reads_per_subset,
                                                  &read_order, i));
        let subset_filename = out_dir.join(format!("sample_{:02}.fastq", i + 1));
        eprintln!("  {}", subset_filename.display());
        let subset_file = File::create(subset_filename).expect("Failed to create subset file");
        subset_files.push(subset_file);
        eprintln!();
    }
    write_subsampled_reads(input_fastq, subset_count, &subset_indices, &mut subset_files)
}


fn get_subsample_indices(subset_count: usize, input_count: usize, reads_per_subset: usize,
                         read_order: &Vec<usize>, i: usize) -> HashSet<usize> {
    // For a given subsample (index i), this function returns a HashSet of the read indices which
    // will go in that subsample.
    let mut subsample_indices = HashSet::new();
    let start_1 = ((i * input_count) as f64 / subset_count as f64).round() as usize;
    let mut end_1 = start_1 + reads_per_subset;
    if end_1 > input_count {
        let start_2 = 0;
        let end_2 = end_1 - input_count;
        end_1 = input_count;
        eprintln!("  reads {}-{} and {}-{}", start_1 + 1, end_1, start_2 + 1, end_2);
        for j in start_2..end_2 {
            subsample_indices.insert(read_order[j]);
        }
    } else {
        eprintln!("  reads {}-{}", start_1 + 1, end_1);
    }
    for j in start_1..end_1 {
        subsample_indices.insert(read_order[j]);
    }
    assert_eq!(subsample_indices.len(), reads_per_subset);
    subsample_indices
}


fn write_subsampled_reads(input_fastq: &PathBuf, subset_count: usize,
                          subset_indices: &Vec<HashSet<usize>>, subset_files: &mut Vec<File>) {
    let pb = spinner("writing subsampled reads to files...");
    let mut read_i = 0;
    let mut reader = fastq_reader(input_fastq);
    while let Some(record) = reader.next() {
        let record = record.expect("Error reading FASTQ file");
        for subset_i in 0..subset_count {
            if subset_indices[subset_i].contains(&read_i) {
                record.write(&subset_files[subset_i]).unwrap();
            }
        }
        read_i += 1;
    }
    pb.finish_and_clear();
}


fn finished_message() {
    section_header("Finished!");
    explanation("You can now assemble each of the subsampled read sets to produce a set of \
                 assemblies for input into Autocycler compress.")
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::panic;

    #[test]
    fn test_parse_genome_size() {
        assert_eq!(parse_genome_size("100"), 100);
        assert_eq!(parse_genome_size("5000"), 5000);
        assert_eq!(parse_genome_size("5000.1"), 5000);
        assert_eq!(parse_genome_size("5000.9"), 5001);
        assert_eq!(parse_genome_size(" 435 "), 435);
        assert_eq!(parse_genome_size("1234567890"), 1234567890);
        assert_eq!(parse_genome_size("12.0k"), 12000);
        assert_eq!(parse_genome_size("47K"), 47000);
        assert_eq!(parse_genome_size("2m"), 2000000);
        assert_eq!(parse_genome_size("13.1M"), 13100000);
        assert_eq!(parse_genome_size("3g"), 3000000000);
        assert_eq!(parse_genome_size("1.23456G"), 1234560000);
        assert!(panic::catch_unwind(|| {
            parse_genome_size("abcd");
        }).is_err());
        assert!(panic::catch_unwind(|| {
            parse_genome_size("12q");
        }).is_err());
        assert!(panic::catch_unwind(|| {
            parse_genome_size("m123");
        }).is_err());
        assert!(panic::catch_unwind(|| {
            parse_genome_size("15kg");
        }).is_err());
    }
}

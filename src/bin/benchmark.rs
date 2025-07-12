use std::time::Instant;
use std::path::PathBuf;
use voicevox_cli::{VoicevoxCore, paths::VoicevoxPaths};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("VOICEVOX Model Loading Benchmark");
    println!("=================================\n");

    // Initialize paths
    let paths = VoicevoxPaths::new();
    
    // Test models
    let test_models = vec![
        (0, "四国めたん（ノーマル）"),
        (1, "ずんだもん（ノーマル）"),
        (8, "春日部つむぎ（ノーマル）"),
        (14, "冥鳴ひまり（ノーマル）"),
    ];
    
    let test_text = "こんにちは、今日はいい天気ですね。ベンチマークテストを実行しています。";
    
    // First, benchmark with caching (load once, synthesize multiple times)
    println!("## Benchmark WITH caching (load once, synthesize 5 times):\n");
    
    for (model_id, model_name) in &test_models {
        let model_path = paths.models_dir.join(format!("vvms/{}.vvm", model_id));
        if !model_path.exists() {
            println!("Model {} not found, skipping", model_id);
            continue;
        }
        
        // Measure model loading time
        let start = Instant::now();
        let mut core = VoicevoxCore::new(&paths)?;
        core.load_model(&model_path)?;
        let load_time = start.elapsed();
        
        // Warm up
        let _ = core.audio_query(test_text, 0)?;
        
        // Measure synthesis time (5 iterations)
        let mut synthesis_times = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            let query = core.audio_query(test_text, 0)?;
            let _audio = core.synthesis(&query, 0)?;
            synthesis_times.push(start.elapsed());
        }
        
        let avg_synthesis = synthesis_times.iter().sum::<std::time::Duration>() / synthesis_times.len() as u32;
        
        println!("Model {} - {}:", model_id, model_name);
        println!("  Load time: {:.2}ms", load_time.as_millis());
        println!("  Avg synthesis time: {:.2}ms", avg_synthesis.as_millis());
        println!("  Total (load + 5 synth): {:.2}ms", 
            (load_time + synthesis_times.iter().sum::<std::time::Duration>()).as_millis());
        println!();
    }
    
    // Second, benchmark without caching (load + synthesize each time)
    println!("\n## Benchmark WITHOUT caching (load + synthesize each time):\n");
    
    for (model_id, model_name) in &test_models {
        let model_path = paths.models_dir.join(format!("vvms/{}.vvm", model_id));
        if !model_path.exists() {
            continue;
        }
        
        let mut total_times = Vec::new();
        
        for _ in 0..5 {
            let start = Instant::now();
            
            // Create new core and load model each time
            let mut core = VoicevoxCore::new(&paths)?;
            core.load_model(&model_path)?;
            
            // Synthesize
            let query = core.audio_query(test_text, 0)?;
            let _audio = core.synthesis(&query, 0)?;
            
            total_times.push(start.elapsed());
            
            // Drop core to simulate no caching
            drop(core);
        }
        
        let avg_total = total_times.iter().sum::<std::time::Duration>() / total_times.len() as u32;
        
        println!("Model {} - {}:", model_id, model_name);
        println!("  Avg total time (load + synth): {:.2}ms", avg_total.as_millis());
        println!("  Total for 5 operations: {:.2}ms", 
            total_times.iter().sum::<std::time::Duration>().as_millis());
        println!();
    }
    
    // Summary
    println!("\n## Summary:\n");
    println!("With caching: Load model once, then synthesize multiple times");
    println!("Without caching: Load model + synthesize for each request");
    println!("\nThe difference shows the benefit of keeping models loaded in memory.");
    
    Ok(())
}
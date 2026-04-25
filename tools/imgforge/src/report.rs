use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Serialize)]
pub struct BuildReport {
    pub tiles_compiled: usize,
    pub tiles_failed: usize,
    pub total_points: usize,
    pub total_polylines: usize,
    pub total_polygons: usize,
    pub duration_ms: u64,
    pub duration_seconds: f64,
    pub output_file: String,
    pub img_size_bytes: u64,
}

impl BuildReport {
    pub fn new() -> Self {
        Self {
            tiles_compiled: 0,
            tiles_failed: 0,
            total_points: 0,
            total_polylines: 0,
            total_polygons: 0,
            duration_ms: 0,
            duration_seconds: 0.0,
            output_file: String::new(),
            img_size_bytes: 0,
        }
    }

    pub fn set_duration(&mut self, duration: Duration) {
        self.duration_ms = duration.as_millis() as u64;
        self.duration_seconds = duration.as_secs_f64();
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn write_json_report(&self, path: &str) -> anyhow::Result<()> {
        let json = self.to_json();
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn print_console_summary(&self) {
        let status_symbol = if self.tiles_failed == 0 { "✅" } else { "⚠️ " };
        let status_text = if self.tiles_failed == 0 { "SUCCÈS" } else { "PARTIEL" };

        println!("\n{} Compilation terminée — Statut: {}", status_symbol, status_text);
        println!("╔════════════════════════════════════════════════════════╗");
        println!("║ RÉSUMÉ D'EXÉCUTION                                     ║");
        println!("╠════════════════════════════════════════════════════════╣");
        println!("║ Tuiles compilées : {:>10}                      ║", self.tiles_compiled);
        println!("║ Tuiles échouées  : {:>10}                      ║", self.tiles_failed);
        println!("║ Points           : {:>10}                      ║", self.total_points);
        println!("║ Polylignes       : {:>10}                      ║", self.total_polylines);
        println!("║ Polygones        : {:>10}                      ║", self.total_polygons);
        println!("║ Taille IMG       : {:>10}                      ║", format_size(self.img_size_bytes));
        println!("║ Durée totale     : {:>7.1} sec                   ║", self.duration_seconds);
        println!("╚════════════════════════════════════════════════════════╝");
        println!("   Fichier de sortie : {}", self.output_file);

        if self.tiles_failed > 0 {
            println!("\n⚠️  {} tuile(s) en échec — carte incomplète", self.tiles_failed);
        }

        println!("\n💡 Astuce : Utilisez -vv pour des logs de débogage détaillés");
    }
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} Mo", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1} Ko", bytes as f64 / 1_024.0)
    } else {
        format!("{} o", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_json() {
        let mut report = BuildReport::new();
        report.tiles_compiled = 1;
        report.total_points = 10;
        report.output_file = "test.img".to_string();
        let json = report.to_json();
        assert!(json.contains("\"tiles_compiled\": 1"));
        assert!(json.contains("\"total_points\": 10"));
        assert!(json.contains("\"tiles_failed\": 0"));
        assert!(json.contains("\"duration_seconds\""));
        assert!(json.contains("\"img_size_bytes\""));
    }

    #[test]
    fn test_set_duration() {
        let mut report = BuildReport::new();
        report.set_duration(Duration::from_millis(2500));
        assert_eq!(report.duration_ms, 2500);
        assert!((report.duration_seconds - 2.5).abs() < 0.001);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(512), "512 o");
        assert_eq!(format_size(2048), "2.0 Ko");
        assert_eq!(format_size(3_145_728), "3.0 Mo");
    }
}

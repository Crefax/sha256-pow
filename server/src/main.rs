use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::io::{self, Read, Write};
use std::time::{Duration, SystemTime};

// İş paketi durumunu tutmak için struct
#[derive(Clone, Debug)]
struct WorkPackage {
    completed: bool,
    assigned_time: Option<SystemTime>,
    assigned_to: Option<String>,
    timeout_count: u32,
}

impl WorkPackage {
    fn new() -> Self {
        WorkPackage {
            completed: false,
            assigned_time: None,
            assigned_to: None,
            timeout_count: 0,
        }
    }

    fn is_timed_out(&self) -> bool {
        if let Some(time) = self.assigned_time {
            if !self.completed {
                time.elapsed().map(|elapsed| elapsed > Duration::from_secs(30)).unwrap_or(true)
            } else {
                false
            }
        } else {
            false
        }
    }

    fn reset_for_timeout(&mut self) {
        self.assigned_time = None;
        self.assigned_to = None;
        self.timeout_count += 1;
    }

    fn mark_completed(&mut self) {
        self.completed = true;
        self.assigned_time = None;
        self.assigned_to = None;
    }
}

// RESULT mesajını işleme fonksiyonu
fn handle_result(ranges: &mut HashMap<u128, WorkPackage>, request: &str, peer_addr: &str) {
    let parts: Vec<&str> = request.split_whitespace().collect();
    
    if request.starts_with("RESULT_EMPTY") {
        if parts.len() >= 3 {
            if let (Ok(range_start), Ok(_)) = (parts[1].parse::<u128>(), parts[2].parse::<u128>()) {
                if let Some(package) = ranges.get_mut(&range_start) {
                    package.mark_completed();
                    println!("[{}] Boş aralık tamamlandı: {} - {}", peer_addr, parts[1], parts[2]);
                }
            }
        }
        return;
    }

    if parts.len() >= 4 {
        let number: u128 = match parts[2].parse() {
            Ok(num) => num,
            Err(e) => {
                println!("Hata: Sayı ayrıştırılamadı: {} - {}", parts[2], e);
                return;
            }
        };
        
        let base_range = (number / 10_000_000) * 10_000_000;
        println!("Debug: Sonuç için base_range hesaplandı: {}", base_range);
        
        if let Some(package) = ranges.get_mut(&base_range) {
            package.mark_completed();
            println!("\nSonuç alındı [{}]:", peer_addr);
            println!("  Kombinasyon: {}", parts[1]);
            println!("  Sayı: {}", parts[2]);
            println!("  Hash: {}", parts[3]);
            println!("  Timeout sayısı: {}", package.timeout_count);
            println!("  İş paketi aralığı: {} - {}", base_range, base_range + 10_000_000);
            println!("-------------------");
        } else {
            println!("Hata: {} sayısı için iş paketi bulunamadı!", base_range);
            println!("Mevcut iş paketleri:");
            for (range, _) in ranges.iter() {
                println!("  {}", range);
            }
        }
    }
}

// İstemci bağlantısını yöneten fonksiyon
fn handle_client(stream: TcpStream, work_ranges: Arc<Mutex<HashMap<u128, WorkPackage>>>) -> io::Result<()> {
    let mut stream = stream;
    let peer_addr = stream.peer_addr()?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;
    
    let mut buffer = [0; 1024];
    
    loop {
        buffer.fill(0);
        
        match stream.read(&mut buffer) {
            Ok(0) => {
                println!("[{}] İstemci bağlantıyı kapattı", peer_addr);
                return Ok(());
            }
            Ok(bytes_read) => {
                let request = String::from_utf8_lossy(&buffer[..bytes_read]).trim().to_string();
                
                if request.starts_with("GET_WORK") {
                    let mut ranges = work_ranges.lock().unwrap();
                    
                    // Timeout olmuş işleri sıfırla
                    for (range_start, package) in ranges.iter_mut() {
                        if package.is_timed_out() {
                            println!("Timeout olan iş sıfırlandı: {} (Timeout sayısı: {})", range_start, package.timeout_count);
                            package.reset_for_timeout();
                        }
                    }
                    
                    // İş seçim stratejisi
                    let available_range = ranges.iter()
                        .filter(|&(_, package)| !package.completed && package.assigned_time.is_none())
                        .min_by_key(|&(range_start, package)| (package.timeout_count, range_start))
                        .map(|(&range_start, _)| range_start);
                    
                    match available_range {
                        Some(range_start) => {
                            let package = ranges.get_mut(&range_start).unwrap();
                            package.assigned_time = Some(SystemTime::now());
                            package.assigned_to = Some(peer_addr.to_string());
                            
                            let range_end = range_start + 10_000_000;
                            let response = format!("{}\n", range_start);
                            stream.write_all(response.as_bytes())?;
                            println!("[{}] İş paketi gönderildi: {} - {} (Timeout sayısı: {})", 
                                   peer_addr, range_start, range_end, package.timeout_count);
                            
                            // İstatistikleri göster
                            let total = ranges.len();
                            let completed = ranges.values().filter(|p| p.completed).count();
                            let in_progress = ranges.values().filter(|p| !p.completed && p.assigned_time.is_some()).count();
                            let available = total - completed - in_progress;
                            
                            println!("\nİş durumu:");
                            println!("  Tamamlanan: {}", completed);
                            println!("  Devam eden: {}", in_progress);
                            println!("  Bekleyen: {}", available);
                            println!("  Timeout sayısı > 0 olan işler:");
                            for (range, package) in ranges.iter() {
                                if package.timeout_count > 0 {
                                    println!("    {} - {}: {} timeout", range, range + 10_000_000, package.timeout_count);
                                }
                            }
                        }
                        None => {
                            let all_completed = ranges.values().all(|p| p.completed);
                            if all_completed {
                                stream.write_all(b"NO_WORK\n")?;
                                println!("[{}] Tüm işler tamamlandı, NO_WORK gönderildi", peer_addr);
                            } else {
                                stream.write_all(b"WAIT\n")?;
                                println!("[{}] Tüm işler atandı, client beklemeye alındı", peer_addr);
                            }
                            return Ok(());
                        }
                    }
                } else if request.starts_with("RESULT") || request.starts_with("RESULT_EMPTY") {
                    let mut ranges = work_ranges.lock().unwrap();
                    handle_result(&mut ranges, &request, &peer_addr.to_string());
                }
            }
            Err(e) => {
                println!("[{}] Okuma hatası: {}", peer_addr, e);
                return Err(e);
            }
        }
    }
}

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:22900")?;
    println!("Sunucu başlatıldı: 127.0.0.1:22900");
    
    let work_ranges = Arc::new(Mutex::new(HashMap::new()));
    let step: u128 = 10_000_000;
    let mut base: u128 = 0;
    
    // İş paketlerini sıralı olarak oluştur
    for i in 0..1000 {
        work_ranges.lock().unwrap().insert(base, WorkPackage::new());
        println!("İş paketi {}: {} - {}", i + 1, base, base + step);
        base += step;
    }
    
    println!("\nİş aralıkları hazırlandı:");
    println!("  Toplam paket sayısı: {}", work_ranges.lock().unwrap().len());
    println!("  Her paket büyüklüğü: {}", step);
    println!("  Toplam aralık: 0 - {}", base);
    println!("\nİstemci bağlantıları bekleniyor...");
    
    // Timeout kontrolü için ayrı bir thread başlat
    let work_ranges_clone = Arc::clone(&work_ranges);
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(10));
            let mut ranges = work_ranges_clone.lock().unwrap();
            let mut timeout_count = 0;
            
            for (range_start, package) in ranges.iter_mut() {
                if package.is_timed_out() {
                    timeout_count += 1;
                    package.reset_for_timeout();
                    println!("Timeout: {} - {} aralığı yeniden dağıtıma açıldı (Timeout sayısı: {})", 
                            range_start, range_start + step, package.timeout_count);
                }
            }
            
            if timeout_count > 0 {
                println!("{} adet timeout olan iş yeniden dağıtıma açıldı", timeout_count);
            }
        }
    });
    
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("\nYeni istemci bağlandı: {}", stream.peer_addr()?);
                let work_ranges = Arc::clone(&work_ranges);
                
                thread::spawn(move || {
                    if let Err(e) = handle_client(stream, work_ranges) {
                        println!("İstemci hatası: {}", e);
                    }
                });
            }
            Err(e) => {
                println!("Bağlantı hatası: {}", e);
            }
        }
    }
    
    Ok(())
}
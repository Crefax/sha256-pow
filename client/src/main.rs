use rayon::prelude::*;
use sha2::{Sha256, Digest};
use std::net::TcpStream;
use std::io::{self, Read, Write};
use std::time::Duration;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

// Sunucuya bağlanma ve yeniden deneme fonksiyonu
fn connect_with_retry(addr: &str, max_retries: u32) -> io::Result<TcpStream> {
    let mut retries = 0;
    loop {
        match TcpStream::connect(addr) {
            Ok(mut stream) => {
                stream.set_read_timeout(Some(Duration::from_secs(30)))?;
                stream.set_write_timeout(Some(Duration::from_secs(30)))?;
                println!("Sunucuya bağlantı başarılı");
                return Ok(stream);
            }
            Err(e) => {
                retries += 1;
                if retries >= max_retries {
                    println!("Maksimum deneme sayısına ulaşıldı");
                    return Err(e);
                }
                println!("Bağlantı başarısız, yeniden deneniyor ({}/{})", retries, max_retries);
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

// Sunucudan iş isteme fonksiyonu
fn get_work(stream: &mut TcpStream) -> io::Result<Option<u128>> {
    stream.write_all(b"GET_WORK\n")?;
    
    let mut buffer = [0; 1024];
    let bytes_read = stream.read(&mut buffer)?;
    
    if bytes_read == 0 {
        return Ok(None);
    }
    
    let response = String::from_utf8_lossy(&buffer[..bytes_read]).trim().to_string();
    println!("Sunucudan gelen yanıt: '{}'", response);
    
    match response.as_str() {
        "NO_WORK" => Ok(None),
        "WAIT" => {
            println!("Sunucu meşgul, 5 saniye bekleniyor...");
            std::thread::sleep(Duration::from_secs(5));
            Ok(Some(0)) // Özel durum: 0 döndürerek tekrar denemesini sağla
        }
        _ => match response.parse() {
            Ok(num) => Ok(Some(num)),
            Err(e) => {
                println!("Sayı ayrıştırma hatası: {}", e);
                Ok(None)
            }
        }
    }
}

// Hash hesaplama ve kontrol fonksiyonu
fn calculate_hash(text: &str, number: u128) -> (String, String) {
    let combined = format!("{}{}", text, number);
    let mut hasher = Sha256::new();
    hasher.update(combined.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    (combined, hash)
}

// Sonuç gönderme fonksiyonu
fn send_result(stream: &mut TcpStream, result: &str) -> io::Result<()> {
    println!("Sunucuya gönderilen sonuç: {}", result);
    stream.write_all(result.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    Ok(())
}

fn main() -> io::Result<()> {
    let text = "Crefax";
    let zeros = 8;
    let prefix = "0".repeat(zeros);
    
    println!("POW Client Başlatılıyor");
    println!("Hedef: {} karakteri için {} adet sıfır ile başlayan hash", text, zeros);
    println!("Aranacak prefix: {}", prefix);
    
    let mut stream = connect_with_retry("127.0.0.1:22900", 5)?;
    let total_hashes_checked = Arc::new(AtomicU64::new(0));
    
    loop {
        match get_work(&mut stream) {
            Ok(Some(range_start)) => {
                if range_start == 0 && total_hashes_checked.load(Ordering::Relaxed) > 0 {
                    // WAIT durumu, tekrar dene
                    continue;
                }

                let range_end = range_start + 10_000_000;
                println!("Aralık taranıyor: {} - {}", range_start, range_end);
                let start_time = std::time::Instant::now();
                let progress_time = Arc::new(std::sync::Mutex::new(std::time::Instant::now()));
                let total_hashes_clone = Arc::clone(&total_hashes_checked);
                
                let found = (range_start..range_end).into_par_iter().find_any(|number| {
                    total_hashes_clone.fetch_add(1, Ordering::Relaxed);
                    
                    let current_count = total_hashes_clone.load(Ordering::Relaxed);
                    if current_count % 1_000_000 == 0 {
                        let mut last_time = progress_time.lock().unwrap();
                        if last_time.elapsed() > Duration::from_secs(5) {
                            *last_time = std::time::Instant::now();
                            println!("İlerleme: {} hash kontrol edildi, şu anki sayı: {}", 
                                   current_count, number);
                        }
                    }
                    
                    let (_, hash) = calculate_hash(text, *number);
                    if hash.starts_with(&prefix) {
                        println!("Potansiyel eşleşme bulundu:");
                        println!("  Sayı: {}", number);
                        println!("  Hash: {}", hash);
                        true
                    } else {
                        false
                    }
                });
                
                let elapsed = start_time.elapsed();
                println!("Aralık tarama tamamlandı: {:.2} saniye", elapsed.as_secs_f64());
                
                if let Some(number) = found {
                    let (combined, hash) = calculate_hash(text, number);
                    let result = format!("RESULT {} {} {}", combined, number, hash);
                    println!("\nSonuç bulundu!");
                    println!("  Kombinasyon: {}", combined);
                    println!("  Sayı: {}", number);
                    println!("  Hash: {}", hash);
                    println!("  Toplam kontrol edilen hash: {}", total_hashes_checked.load(Ordering::Relaxed));
                    
                    match send_result(&mut stream, &result) {
                        Ok(_) => {
                            println!("Sonuç başarıyla gönderildi");
                            break;
                        }
                        Err(e) => {
                            println!("Sonuç gönderme hatası: {}", e);
                            break;
                        }
                    }
                } else {
                    // Aralıkta sonuç bulunamadı, tamamlandı olarak işaretle
                    let result = format!("RESULT_EMPTY {} {}", range_start, range_end);
                    if let Err(e) = send_result(&mut stream, &result) {
                        println!("Boş sonuç gönderme hatası: {}", e);
                        break;
                    }
                }
            }
            Ok(None) => {
                println!("Çalışılacak iş kalmadı, program sonlandırılıyor");
                break;
            }
            Err(e) => {
                println!("Sunucu iletişim hatası: {}", e);
                break;
            }
        }
    }
    
    println!("Program sonlandı. Toplam kontrol edilen hash sayısı: {}", 
             total_hashes_checked.load(Ordering::Relaxed));
    Ok(())
}
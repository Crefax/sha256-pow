# Rust Proof of Work (PoW) Sistemi

Bu proje, dağıtık bir Proof of Work (PoW) sistemi implementasyonudur. Sistem, client-server mimarisi kullanarak SHA-256 hash algoritması ile belirli bir zorluk seviyesinde hash hesaplaması yapar.

## Geliştirme Ortamı

- Cursor IDE kullanılarak geliştirilmiştir
- Rust programlama dili
- Rayon kütüphanesi (paralel işlemler için)
- SHA-256 hash algoritması

## Sistem Mimarisi

### Server
- İş paketlerini yönetir ve dağıtır
- Her iş paketi 10 milyon sayılık bir aralığı kapsar
- Timeout kontrolü ile yarım kalan işleri tekrar dağıtır
- İş durumlarını ve istatistikleri takip eder

### Client
- Sunucudan iş paketi alır
- Paralel işlem ile hash hesaplaması yapar
- Belirtilen zorluk seviyesinde (sıfır sayısı) hash arar
- Sonuçları sunucuya bildirir

## Özellikler

- Multi-threaded çalışma
- Paralel hash hesaplama
- Otomatik iş dağıtımı
- Timeout yönetimi
- İş tamamlanma takibi
- Detaylı loglama
- İlerleme göstergesi

## Kurulum

1. Rust ve Cargo'yu yükleyin
2. Projeyi klonlayın
3. Server ve client klasörlerinde bağımlılıkları yükleyin:
```bash
cd server
cargo build
cd ../client
cargo build
```

## Çalıştırma

1. Önce server'ı başlatın:
```bash
cd server
cargo run
```

2. Yeni bir terminal açın ve client'ı başlatın:
```bash
cd client
cargo run
```

Birden fazla client aynı anda çalıştırılabilir.

## Konfigürasyon

### Server
- Port: 22900 (varsayılan)
- İş paketi büyüklüğü: 10 milyon
- Timeout süresi: 300 saniye

### Client
- Hash zorluk seviyesi: Başlangıçta 8 sıfır
- Bağlantı deneme sayısı: 5
- İlerleme gösterme aralığı: Her 1 milyon hash'te bir

## Teknik Detaylar

- TCP/IP tabanlı iletişim
- SHA-256 hash algoritması
- Rayon ile paralel işlem
- Arc ve Mutex ile thread-safe veri yapıları
- Atomik sayaçlar
- Hata yönetimi ve geri bildirim sistemi

## Lisans

MIT License

## Katkıda Bulunma

1. Fork edin
2. Feature branch oluşturun
3. Değişikliklerinizi commit edin
4. Branch'inizi push edin
5. Pull request açın

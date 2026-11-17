// Anka Browser - URL Veri Yapısı (WHATWG URL Standardı Temeli)

#[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Url {
        //protokol örn. http https

        pub scheme : String,

        //alan adı yahut protokol numarası
        pub host : String,

        //port kapı numarası
        pub port : Option<u16>,

        //dosya yolu
        pub path : String,

        //soru işaretinden sonraki sorgu
        pub query : Option<String>,

        //#işaretinden sonraki sayfa bölümü kısmı
        pub fragment : Option<String>,

    }
// Anka Browser - Sıfırdan JavaScript Motoru ve Canlı DOM API (İstasyon 7 - v0.2.7)
// Hiçbir üçüncü parti JS kütüphanesi (V8, QuickJS, Boa vb.) kullanmadan,
// kendi Lexer (sözcük ayrıştırıcı), ifade değerlendirici (expression evaluator),
// fonksiyon ortamı ve DOM köprümüzü saf Rust ile sıfırdan inşa ediyoruz!

use std::collections::HashMap;

use anka_html::DomNode;

/// JavaScript dilindeki temel veri tipleri:
#[derive(Debug, Clone, PartialEq)]
pub enum JsValue {
    Number(f64),
    Str(String),
    Bool(bool),
    /// `document.getElementById("sayac")` çağrısından dönen canlı DOM öğesi referansı!
    ElementRef(String),
    Undefined,
}

impl JsValue {
    /// Bir JavaScript değerini ekranda / metin birleştirmede görünecek yazıya çevirir.
    pub fn to_js_string(&self) -> String {
        match self {
            JsValue::Number(n) => {
                if n.fract() == 0.0 {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            JsValue::Str(s) => s.clone(),
            JsValue::Bool(b) => b.to_string(),
            JsValue::ElementRef(id) => format!("[Element #{id}]"),
            JsValue::Undefined => "undefined".to_string(),
        }
    }

    /// Bir JavaScript değerinin `if (...)` içinde doğru (`true`) sayılıp sayılmayacağını belirler.
    pub fn is_truthy(&self) -> bool {
        match self {
            JsValue::Bool(b) => *b,
            JsValue::Number(n) => *n != 0.0 && !n.is_nan(),
            JsValue::Str(s) => !s.is_empty(),
            JsValue::ElementRef(_) => true,
            JsValue::Undefined => false,
        }
    }

    /// Sayısal işlemlerde değeri `f64` sayısına çevirir.
    pub fn to_number(&self) -> f64 {
        match self {
            JsValue::Number(n) => *n,
            JsValue::Bool(true) => 1.0,
            JsValue::Bool(false) => 0.0,
            JsValue::Str(s) => s.trim().parse::<f64>().unwrap_or(0.0),
            JsValue::ElementRef(_) | JsValue::Undefined => 0.0,
        }
    }
}

/// Sözcük Ayrıştırıcının (Lexer) ürettiği atomik JavaScript parçacıkları (Token):
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Let,
    Const,
    Var,
    Function,
    If,
    Else,
    Return,
    True,
    False,
    Ident(String),
    Number(f64),
    Str(String),
    Plus,
    Minus,
    Star,
    Slash,
    PlusPlus,
    MinusMinus,
    Eq,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Dot,
    Comma,
    Semicolon,
}

/// Kullanıcı tarafından tanımlanan bir JavaScript fonksiyonu (`function artir() { ... }`):
#[derive(Debug, Clone)]
struct JsFunction {
    params: Vec<String>,
    body: Vec<Token>,
}

/// Ham JavaScript kaynak kodunu token listesine dönüştüren Lexer (Sözcük Ayrıştırıcı):
fn tokenize(src: &str) -> Vec<Token> {
    let chars: Vec<char> = src.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut tokens = Vec::new();

    while i < len {
        let ch = chars[i];

        // 1. Boşlukları atla
        if ch.is_whitespace() {
            i += 1;
            continue;
        }

        // 2. Tek satırlık yorum (`// ...`)
        if ch == '/' && i + 1 < len && chars[i + 1] == '/' {
            i += 2;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // 3. Çok satırlık yorum (`/* ... */`)
        if ch == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(len);
            continue;
        }

        // 4. Metin sabitleri (`"..."` veya `'...'`)
        if ch == '"' || ch == '\'' {
            let quote = ch;
            i += 1;
            let mut s = String::new();
            while i < len && chars[i] != quote {
                if chars[i] == '\\' && i + 1 < len {
                    i += 1;
                    match chars[i] {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        other => s.push(other),
                    }
                } else {
                    s.push(chars[i]);
                }
                i += 1;
            }
            if i < len {
                i += 1; // Kapanış tırnağını geç
            }
            tokens.push(Token::Str(s));
            continue;
        }

        // 5. Sayılar (`42`, `3.14`)
        if ch.is_ascii_digit() {
            let start = i;
            while i < len && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let num_str: String = chars[start..i].iter().collect();
            let val = num_str.parse::<f64>().unwrap_or(0.0);
            tokens.push(Token::Number(val));
            continue;
        }

        // 6. Anahtar kelimeler ve değişken/fonksiyon adları (`let`, `document`, `sayac`)
        if ch.is_alphabetic() || ch == '_' || ch == '$' {
            let start = i;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$') {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();
            let tok = match ident.as_str() {
                "let" => Token::Let,
                "const" => Token::Const,
                "var" => Token::Var,
                "function" => Token::Function,
                "if" => Token::If,
                "else" => Token::Else,
                "return" => Token::Return,
                "true" => Token::True,
                "false" => Token::False,
                _ => Token::Ident(ident),
            };
            tokens.push(tok);
            continue;
        }

        // 7. Çok karakterli ve tek karakterli operatörler
        match ch {
            '+' => {
                if i + 1 < len && chars[i + 1] == '+' {
                    tokens.push(Token::PlusPlus);
                    i += 2;
                } else {
                    tokens.push(Token::Plus);
                    i += 1;
                }
            }
            '-' => {
                if i + 1 < len && chars[i + 1] == '-' {
                    tokens.push(Token::MinusMinus);
                    i += 2;
                } else {
                    tokens.push(Token::Minus);
                    i += 1;
                }
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '=' => {
                if i + 2 < len && chars[i + 1] == '=' && chars[i + 2] == '=' {
                    tokens.push(Token::EqEq);
                    i += 3;
                } else if i + 1 < len && chars[i + 1] == '=' {
                    tokens.push(Token::EqEq);
                    i += 2;
                } else {
                    tokens.push(Token::Eq);
                    i += 1;
                }
            }
            '!' => {
                if i + 2 < len && chars[i + 1] == '=' && chars[i + 2] == '=' {
                    tokens.push(Token::NotEq);
                    i += 3;
                } else if i + 1 < len && chars[i + 1] == '=' {
                    tokens.push(Token::NotEq);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            '<' => {
                if i + 1 < len && chars[i + 1] == '=' {
                    tokens.push(Token::Le);
                    i += 2;
                } else {
                    tokens.push(Token::Lt);
                    i += 1;
                }
            }
            '>' => {
                if i + 1 < len && chars[i + 1] == '=' {
                    tokens.push(Token::Ge);
                    i += 2;
                } else {
                    tokens.push(Token::Gt);
                    i += 1;
                }
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '{' => {
                tokens.push(Token::LBrace);
                i += 1;
            }
            '}' => {
                tokens.push(Token::RBrace);
                i += 1;
            }
            '.' => {
                tokens.push(Token::Dot);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            ';' => {
                tokens.push(Token::Semicolon);
                i += 1;
            }
            _ => {
                // Desteklenmeyen karakterleri çökmeden atla (hata toleransı)
                i += 1;
            }
        }
    }

    tokens
}

/// Her bir tarayıcı sekmesine ait canlı JavaScript çalışma ortamı (Runtime):
/// Değişkenleri (`let sayac = 0`), fonksiyonları (`function artir() { ... }`),
/// `document.title` güncellemesini ve `console.log(...)` kayıtlarını saklar!
#[derive(Debug, Clone, Default)]
pub struct JsRuntime {
    pub variables: HashMap<String, JsValue>,
    functions: HashMap<String, JsFunction>,
    pub document_title: Option<String>,
    pub logs: Vec<String>,
}

impl JsRuntime {
    /// Yeni ve boş bir JavaScript çalışma zamanı oluşturur.
    pub fn new() -> Self {
        Self::default()
    }

    /// Verilen JavaScript kodunu bu sekmenin canlı DOM ağacı (`dom: &mut DomNode`)
    /// üzerinde çalıştırır. DOM'da bir değişiklik yapıldıysa veya kod başarıyla yürütüldüyse
    /// dönen son değeri (`JsValue`) verir.
    pub fn eval(&mut self, code: &str, dom: &mut DomNode) -> JsValue {
        let tokens = tokenize(code);
        let mut steps = 0usize;
        self.exec_block(&tokens, dom, &mut steps)
    }

    /// Bir token bloğunu sırayla çalıştırır. `steps` sayacı sonsuz döngülere karşı güvenlik sınırıdır.
    fn exec_block(&mut self, tokens: &[Token], dom: &mut DomNode, steps: &mut usize) -> JsValue {
        let mut pos = 0;
        let mut last_val = JsValue::Undefined;

        while pos < tokens.len() && *steps < 10_000 {
            *steps += 1;

            match &tokens[pos] {
                Token::Semicolon => {
                    pos += 1;
                }

                // 1. `let x = ...;` / `const x = ...;` / `var x = ...;`
                Token::Let | Token::Const | Token::Var => {
                    pos += 1;
                    if let Some(Token::Ident(name)) = tokens.get(pos).cloned() {
                        pos += 1;
                        let val = if matches!(tokens.get(pos), Some(Token::Eq)) {
                            pos += 1;
                            self.eval_expr(tokens, &mut pos, dom, steps)
                        } else {
                            JsValue::Undefined
                        };
                        self.variables.insert(name, val.clone());
                        last_val = val;
                    }
                    if matches!(tokens.get(pos), Some(Token::Semicolon)) {
                        pos += 1;
                    }
                }

                // 2. `function ad(p1, p2) { ... }`
                Token::Function => {
                    pos += 1;
                    if let Some(Token::Ident(fn_name)) = tokens.get(pos).cloned() {
                        pos += 1;
                        let mut params = Vec::new();
                        if matches!(tokens.get(pos), Some(Token::LParen)) {
                            pos += 1;
                            while pos < tokens.len() && !matches!(tokens[pos], Token::RParen) {
                                if let Token::Ident(p) = &tokens[pos] {
                                    params.push(p.clone());
                                }
                                pos += 1;
                            }
                            if matches!(tokens.get(pos), Some(Token::RParen)) {
                                pos += 1;
                            }
                        }
                        if matches!(tokens.get(pos), Some(Token::LBrace)) {
                            let body = extract_brace_block(tokens, &mut pos);
                            self.functions.insert(fn_name, JsFunction { params, body });
                        }
                    }
                }

                // 3. `if (kosul) { ... } else { ... }`
                Token::If => {
                    pos += 1;
                    let cond = if matches!(tokens.get(pos), Some(Token::LParen)) {
                        pos += 1;
                        let v = self.eval_expr(tokens, &mut pos, dom, steps);
                        if matches!(tokens.get(pos), Some(Token::RParen)) {
                            pos += 1;
                        }
                        v.is_truthy()
                    } else {
                        false
                    };

                    let then_block = if matches!(tokens.get(pos), Some(Token::LBrace)) {
                        extract_brace_block(tokens, &mut pos)
                    } else {
                        Vec::new()
                    };

                    let else_block = if matches!(tokens.get(pos), Some(Token::Else)) {
                        pos += 1;
                        if matches!(tokens.get(pos), Some(Token::LBrace)) {
                            extract_brace_block(tokens, &mut pos)
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };

                    if cond {
                        last_val = self.exec_block(&then_block, dom, steps);
                    } else if !else_block.is_empty() {
                        last_val = self.exec_block(&else_block, dom, steps);
                    }
                }

                // 4. `return ifade;`
                Token::Return => {
                    pos += 1;
                    return self.eval_expr(tokens, &mut pos, dom, steps);
                }

                // 5. İfade veya Atama Deyimi (`x = x + 1;`, `x++;`, `document.getElementById("a").innerText = "Selam";`)
                _ => {
                    last_val = self.exec_statement_expr(tokens, &mut pos, dom, steps);
                    if matches!(tokens.get(pos), Some(Token::Semicolon)) {
                        pos += 1;
                    }
                }
            }
        }

        last_val
    }

    /// Atamaları (`x = 5`, `x++`, `document.title = ...`, `el.innerText = ...`, `el.style.color = ...`)
    /// veya serbest fonksiyon çağrılarını (`console.log(...)`, `artir()`) yürütür.
    fn exec_statement_expr(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
        dom: &mut DomNode,
        steps: &mut usize,
    ) -> JsValue {
        // A) Basit değişken artırma/azaltma veya atama: `sayac++`, `sayac--`, `sayac = ...`
        if let Some(Token::Ident(var_name)) = tokens.get(*pos).cloned() {
            match tokens.get(*pos + 1) {
                Some(Token::PlusPlus) => {
                    *pos += 2;
                    let next_num = self
                        .variables
                        .get(&var_name)
                        .map_or(0.0, JsValue::to_number)
                        + 1.0;
                    let val = JsValue::Number(next_num);
                    self.variables.insert(var_name, val.clone());
                    return val;
                }
                Some(Token::MinusMinus) => {
                    *pos += 2;
                    let next_num = self
                        .variables
                        .get(&var_name)
                        .map_or(0.0, JsValue::to_number)
                        - 1.0;
                    let val = JsValue::Number(next_num);
                    self.variables.insert(var_name, val.clone());
                    return val;
                }
                Some(Token::Eq) => {
                    *pos += 2;
                    let val = self.eval_expr(tokens, pos, dom, steps);
                    self.variables.insert(var_name, val.clone());
                    return val;
                }
                _ => {}
            }
        }

        // B) `document.title = "..."`
        if let (Some(Token::Ident(obj)), Some(Token::Dot), Some(Token::Ident(prop)), Some(Token::Eq)) =
            (
                tokens.get(*pos),
                tokens.get(*pos + 1),
                tokens.get(*pos + 2),
                tokens.get(*pos + 3),
            )
            && obj == "document"
            && prop == "title"
        {
            *pos += 4;
            let val = self.eval_expr(tokens, pos, dom, steps);
            self.document_title = Some(val.to_js_string());
            return val;
        }

        // C) `document.getElementById("id").innerText = ...` veya `.style.color = ...`
        // veya değişken üzerinden `kutu.innerText = ...` / `kutu.style.color = ...`
        let saved_pos = *pos;
        if let Some(target_id) = self.try_parse_element_target(tokens, pos, dom, steps) {
            // `.innerText = ...` veya `.textContent = ...`
            if let (Some(Token::Dot), Some(Token::Ident(prop)), Some(Token::Eq)) = (
                tokens.get(*pos),
                tokens.get(*pos + 1),
                tokens.get(*pos + 2),
            ) && (prop == "innerText" || prop == "textContent" || prop == "innerHTML")
            {
                *pos += 3;
                let val = self.eval_expr(tokens, pos, dom, steps);
                let text = val.to_js_string();
                if let Some(node) = dom.find_by_id_mut(&target_id) {
                    node.set_text(&text);
                }
                return val;
            }

            // `.style.color = ...` veya `.style.backgroundColor = ...`
            if let (
                Some(Token::Dot),
                Some(Token::Ident(style_kw)),
                Some(Token::Dot),
                Some(Token::Ident(css_prop)),
                Some(Token::Eq),
            ) = (
                tokens.get(*pos),
                tokens.get(*pos + 1),
                tokens.get(*pos + 2),
                tokens.get(*pos + 3),
                tokens.get(*pos + 4),
            ) && style_kw == "style"
            {
                let css_key = match css_prop.as_str() {
                    "backgroundColor" => "background-color".to_string(),
                    "fontSize" => "font-size".to_string(),
                    "borderColor" => "border-color".to_string(),
                    other => other.to_lowercase(),
                };
                *pos += 5;
                let val = self.eval_expr(tokens, pos, dom, steps);
                let css_val = val.to_js_string();
                if let Some(node) = dom.find_by_id_mut(&target_id) {
                    let mevcut = node.get_attribute("style").unwrap_or("").to_string();
                    let yeni_stil = if mevcut.trim().is_empty() {
                        format!("{css_key}: {css_val};")
                    } else {
                        format!("{mevcut}; {css_key}: {css_val};")
                    };
                    node.set_attribute("style", &yeni_stil);
                }
                return val;
            }
        }

        // Atama deseni değilse pozisyonu geri sar ve normal ifade olarak değerlendir
        *pos = saved_pos;
        self.eval_expr(tokens, pos, dom, steps)
    }

    /// `document.getElementById("...")` veya `ElementRef` tutan bir değişken adını yakalayıp ID döndürür.
    fn try_parse_element_target(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
        dom: &mut DomNode,
        steps: &mut usize,
    ) -> Option<String> {
        // 1. `document.getElementById("...")`
        if let (
            Some(Token::Ident(doc)),
            Some(Token::Dot),
            Some(Token::Ident(method)),
            Some(Token::LParen),
        ) = (
            tokens.get(*pos),
            tokens.get(*pos + 1),
            tokens.get(*pos + 2),
            tokens.get(*pos + 3),
        ) && doc == "document"
            && method == "getElementById"
        {
            *pos += 4;
            let arg = self.eval_expr(tokens, pos, dom, steps);
            if matches!(tokens.get(*pos), Some(Token::RParen)) {
                *pos += 1;
            }
            return Some(arg.to_js_string());
        }

        // 2. Daha önce `let kutu = document.getElementById("...");` ile saklanmış değişken
        if let Some(Token::Ident(var_name)) = tokens.get(*pos)
            && let Some(JsValue::ElementRef(id)) = self.variables.get(var_name).cloned()
        {
            *pos += 1;
            return Some(id);
        }

        None
    }

    /// İfade değerlendirme (Karşılaştırma: `==`, `!=`, `<`, `<=`, `>`, `>=`)
    fn eval_expr(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
        dom: &mut DomNode,
        steps: &mut usize,
    ) -> JsValue {
        let left = self.eval_additive(tokens, pos, dom, steps);
        if let Some(op) = tokens.get(*pos).cloned() {
            match op {
                Token::EqEq | Token::NotEq | Token::Lt | Token::Le | Token::Gt | Token::Ge => {
                    *pos += 1;
                    let right = self.eval_additive(tokens, pos, dom, steps);
                    let res = match op {
                        Token::EqEq => left.to_js_string() == right.to_js_string(),
                        Token::NotEq => left.to_js_string() != right.to_js_string(),
                        Token::Lt => left.to_number() < right.to_number(),
                        Token::Le => left.to_number() <= right.to_number(),
                        Token::Gt => left.to_number() > right.to_number(),
                        Token::Ge => left.to_number() >= right.to_number(),
                        _ => false,
                    };
                    return JsValue::Bool(res);
                }
                _ => {}
            }
        }
        left
    }

    /// Toplama, çıkarma ve JavaScript metin birleştirme (`"Sayi: " + 5` -> `"Sayi: 5"`)
    fn eval_additive(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
        dom: &mut DomNode,
        steps: &mut usize,
    ) -> JsValue {
        let mut left = self.eval_multiplicative(tokens, pos, dom, steps);

        while let Some(op) = tokens.get(*pos).cloned() {
            match op {
                Token::Plus => {
                    *pos += 1;
                    let right = self.eval_multiplicative(tokens, pos, dom, steps);
                    left = match (&left, &right) {
                        (JsValue::Str(_), _) | (_, JsValue::Str(_)) => {
                            JsValue::Str(format!("{}{}", left.to_js_string(), right.to_js_string()))
                        }
                        _ => JsValue::Number(left.to_number() + right.to_number()),
                    };
                }
                Token::Minus => {
                    *pos += 1;
                    let right = self.eval_multiplicative(tokens, pos, dom, steps);
                    left = JsValue::Number(left.to_number() - right.to_number());
                }
                _ => break,
            }
        }

        left
    }

    /// Çarpma ve bölme (`*`, `/`)
    fn eval_multiplicative(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
        dom: &mut DomNode,
        steps: &mut usize,
    ) -> JsValue {
        let mut left = self.eval_primary(tokens, pos, dom, steps);

        while let Some(op) = tokens.get(*pos).cloned() {
            match op {
                Token::Star => {
                    *pos += 1;
                    let right = self.eval_primary(tokens, pos, dom, steps);
                    left = JsValue::Number(left.to_number() * right.to_number());
                }
                Token::Slash => {
                    *pos += 1;
                    let right = self.eval_primary(tokens, pos, dom, steps);
                    let denom = right.to_number();
                    left = if denom == 0.0 {
                        JsValue::Number(0.0)
                    } else {
                        JsValue::Number(left.to_number() / denom)
                    };
                }
                _ => break,
            }
        }

        left
    }

    /// Birincil ifadeler: sayılar, metinler, değişkenler, `console.log`, `document.getElementById`, fonksiyon çağrıları
    fn eval_primary(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
        dom: &mut DomNode,
        steps: &mut usize,
    ) -> JsValue {
        let Some(tok) = tokens.get(*pos).cloned() else {
            return JsValue::Undefined;
        };

        match tok {
            Token::Number(n) => {
                *pos += 1;
                JsValue::Number(n)
            }
            Token::Str(s) => {
                *pos += 1;
                JsValue::Str(s)
            }
            Token::True => {
                *pos += 1;
                JsValue::Bool(true)
            }
            Token::False => {
                *pos += 1;
                JsValue::Bool(false)
            }
            Token::Minus => {
                *pos += 1;
                let v = self.eval_primary(tokens, pos, dom, steps);
                JsValue::Number(-v.to_number())
            }
            Token::LParen => {
                *pos += 1;
                let v = self.eval_expr(tokens, pos, dom, steps);
                if matches!(tokens.get(*pos), Some(Token::RParen)) {
                    *pos += 1;
                }
                v
            }
            Token::Ident(name) => {
                *pos += 1;

                // 1. `console.log(...)`
                if name == "console"
                    && matches!(tokens.get(*pos), Some(Token::Dot))
                    && let Some(Token::Ident(m)) = tokens.get(*pos + 1).cloned()
                    && m == "log"
                    && matches!(tokens.get(*pos + 2), Some(Token::LParen))
                {
                    *pos += 3;
                    let args = self.parse_call_args(tokens, pos, dom, steps);
                    let msg: Vec<String> = args.iter().map(JsValue::to_js_string).collect();
                    self.logs.push(msg.join(" "));
                    return JsValue::Undefined;
                }

                // 2. `document.getElementById("...")` (+ isteğe bağlı `.innerText` okuması)
                if name == "document" && matches!(tokens.get(*pos), Some(Token::Dot)) {
                    *pos += 1;
                    if let Some(Token::Ident(prop)) = tokens.get(*pos).cloned() {
                        *pos += 1;
                        if prop == "getElementById" && matches!(tokens.get(*pos), Some(Token::LParen))
                        {
                            *pos += 1;
                            let args = self.parse_call_args(tokens, pos, dom, steps);
                            let id = args.first().map_or_else(String::new, JsValue::to_js_string);
                            // Ardından `.innerText` veya `.textContent` geliyorsa doğrudan metni oku:
                            if matches!(tokens.get(*pos), Some(Token::Dot))
                                && let Some(Token::Ident(sub)) = tokens.get(*pos + 1).cloned()
                                && (sub == "innerText" || sub == "textContent")
                            {
                                *pos += 2;
                                let txt = dom
                                    .find_by_id_mut(&id)
                                    .map_or_else(String::new, |n| n.get_text());
                                return JsValue::Str(txt);
                            }
                            return JsValue::ElementRef(id);
                        }
                        if prop == "title" {
                            return JsValue::Str(
                                self.document_title.clone().unwrap_or_default(),
                            );
                        }
                    }
                    return JsValue::Undefined;
                }

                // 3. Kullanıcı tanımlı fonksiyon çağrısı: `artir()` veya `topla(3, 5)`
                if matches!(tokens.get(*pos), Some(Token::LParen)) {
                    *pos += 1;
                    let args = self.parse_call_args(tokens, pos, dom, steps);
                    if let Some(func) = self.functions.get(&name).cloned() {
                        // Parametreleri geçici olarak bağla ve fonksiyon gövdesini çalıştır
                        let mut saved = Vec::new();
                        for (idx, param) in func.params.iter().enumerate() {
                            saved.push((param.clone(), self.variables.get(param).cloned()));
                            let arg_val = args.get(idx).cloned().unwrap_or(JsValue::Undefined);
                            self.variables.insert(param.clone(), arg_val);
                        }
                        let ret = self.exec_block(&func.body, dom, steps);
                        for (param, old_val) in saved {
                            match old_val {
                                Some(v) => {
                                    self.variables.insert(param, v);
                                }
                                None => {
                                    self.variables.remove(&param);
                                }
                            }
                        }
                        return ret;
                    }
                    return JsValue::Undefined;
                }

                // 4. Değişken okuması (+ eğer `ElementRef` ise `.innerText` okuması)
                let val = self
                    .variables
                    .get(&name)
                    .cloned()
                    .unwrap_or(JsValue::Undefined);
                if let JsValue::ElementRef(ref id) = val
                    && matches!(tokens.get(*pos), Some(Token::Dot))
                    && let Some(Token::Ident(sub)) = tokens.get(*pos + 1).cloned()
                    && (sub == "innerText" || sub == "textContent")
                {
                    *pos += 2;
                    let txt = dom
                        .find_by_id_mut(id)
                        .map_or_else(String::new, |n| n.get_text());
                    return JsValue::Str(txt);
                }
                val
            }
            _ => {
                *pos += 1;
                JsValue::Undefined
            }
        }
    }

    /// Parantez içindeki virgülle ayrılmış fonksiyon argümanlarını değerlendirir (`(a, b + 1)`).
    fn parse_call_args(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
        dom: &mut DomNode,
        steps: &mut usize,
    ) -> Vec<JsValue> {
        let mut args = Vec::new();
        while *pos < tokens.len() && !matches!(tokens[*pos], Token::RParen) {
            args.push(self.eval_expr(tokens, pos, dom, steps));
            if matches!(tokens.get(*pos), Some(Token::Comma)) {
                *pos += 1;
            } else {
                break;
            }
        }
        if matches!(tokens.get(*pos), Some(Token::RParen)) {
            *pos += 1;
        }
        args
    }
}

/// `{ ... }` süslü parantez bloğunun içindeki token dilimini iç içe blokları sayarak ayıklar.
fn extract_brace_block(tokens: &[Token], pos: &mut usize) -> Vec<Token> {
    if !matches!(tokens.get(*pos), Some(Token::LBrace)) {
        return Vec::new();
    }
    *pos += 1;
    let start = *pos;
    let mut depth = 1usize;

    while *pos < tokens.len() && depth > 0 {
        match tokens[*pos] {
            Token::LBrace => depth += 1,
            Token::RBrace => depth -= 1,
            _ => {}
        }
        *pos += 1;
    }

    let end = if depth == 0 { *pos - 1 } else { *pos };
    tokens[start..end].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anka_html::parse_html;

    #[test]
    fn test_degisken_aritmetik_ve_console_log() {
        let mut dom = parse_html("<body></body>");
        let mut js = JsRuntime::new();

        js.eval(
            r#"
            let a = 10;
            let b = 5 * 3;
            let toplam = a + b;
            console.log("Sonuc:", toplam);
            "#,
            &mut dom,
        );

        assert_eq!(js.variables.get("toplam"), Some(&JsValue::Number(25.0)));
        assert_eq!(js.logs, vec!["Sonuc: 25".to_string()]);
    }

    #[test]
    fn test_dom_getelementbyid_innertext_ve_style_guncelleme() {
        let mut dom = parse_html("<body><p id=\"mesaj\">Eski</p></body>");
        let mut js = JsRuntime::new();

        js.eval(
            r##"
            document.title = "Canli Baslik";
            document.getElementById("mesaj").innerText = "Anka JS Calisiyor!";
            document.getElementById("mesaj").style.color = "#dc2626";
            "##,
            &mut dom,
        );

        assert_eq!(js.document_title.as_deref(), Some("Canli Baslik"));
        let mesaj = dom.find_by_id_mut("mesaj").unwrap();
        assert_eq!(mesaj.get_text(), "Anka JS Calisiyor!");
        assert!(mesaj.get_attribute("style").unwrap().contains("color: #dc2626"));
    }

    #[test]
    fn test_fonksiyon_if_else_ve_canli_sayac_onclick() {
        let mut dom = parse_html("<body><p id=\"sayac\">Sayac: 0</p></body>");
        let mut js = JsRuntime::new();

        // Sayfa yüklenirken `<script>` tanımı çalıştırılır
        js.eval(
            r#"
            let n = 0;
            function artir() {
                n++;
                if (n >= 2) {
                    document.getElementById("sayac").innerText = "Harika! Sayac: " + n;
                } else {
                    document.getElementById("sayac").innerText = "Sayac: " + n;
                }
            }
            "#,
            &mut dom,
        );

        // Kullanıcı butona 1. kez tıklar (`onclick="artir()"`)
        js.eval("artir()", &mut dom);
        assert_eq!(dom.find_by_id_mut("sayac").unwrap().get_text(), "Sayac: 1");

        // Kullanıcı butona 2. kez tıklar (`onclick="artir()"`)
        js.eval("artir()", &mut dom);
        assert_eq!(
            dom.find_by_id_mut("sayac").unwrap().get_text(),
            "Harika! Sayac: 2"
        );
    }
}

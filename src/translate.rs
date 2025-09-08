/*
 *  translate.rs
 * 
 *  LyMonS - worth the squeeze
 *	(c) 2020-25 Stuart Hunter
 *
 *	TODO:
 *
 *	This program is free software: you can redistribute it and/or modify
 *	it under the terms of the GNU General Public License as published by
 *	the Free Software Foundation, either version 3 of the License, or
 *	(at your option) any later version.
 *
 *	This program is distributed in the hope that it will be useful,
 *	but WITHOUT ANY WARRANTY; without even the implied warranty of
 *	MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *	GNU General Public License for more details.
 *
 *	See <http://www.gnu.org/licenses/> to get a copy of the GNU General
 *	Public License.
 *
 */
use mini_moka::sync::Cache;
use std::{sync::Arc};
use translators::{GoogleTranslator, Translator};

#[derive(Clone)]
pub struct Translation {
    google_trans: GoogleTranslator,
    to_language: String,
    cache: Arc<Cache<String, String>>,
}

#[allow(dead_code)]
impl Translation {

    pub fn new(to_lang: &str) -> anyhow::Result<Self> {

        let to_language = if !to_lang.is_empty() {to_lang}else{""};
        let cache = Arc::new(Cache::new(50)); // expect 30 phrases max

        Ok(Self {
            google_trans: GoogleTranslator::default(),
            to_language:to_language.to_string(),
            cache,
        })

    }

    pub async fn translate_phrase(&mut self, phrase: &str) -> anyhow::Result<String> {
        let key = format!("{}-{}",self.to_language, phrase);

        if let Some(result) = self.cache.get(&key) {
            return Ok(result);
        }

        let value = self.google_trans
            .translate_async(phrase, "", self.to_language.as_str())
            .await.unwrap();
        self.cache.insert(key.clone(), value.clone());
        Ok(value)
    }

    pub fn translate_phrase_sync(&mut self, phrase: &str) -> anyhow::Result<String> {
        let key = format!("{}-{}",self.to_language, phrase);

        if let Some(result) = self.cache.get(&key) {
            return Ok(result);
        }

        let value = self.google_trans
            .translate_sync(phrase, "", self.to_language.as_str())
            .unwrap();
        self.cache.insert(key.clone(), value.clone());
        Ok(value)
    }

    pub async fn translate_phrase_no_cache(&mut self, phrase: &str) -> anyhow::Result<String> {
        let value = self.google_trans
            .translate_async(phrase, "", self.to_language.as_str())
            .await.unwrap();
        Ok(value)
    }

    pub fn translate_phrase_no_cache_sync(&mut self, phrase: &str) -> anyhow::Result<String> {
        let value = self.google_trans
            .translate_sync(phrase, "", self.to_language.as_str())
            .unwrap();
        Ok(value)
    }


}


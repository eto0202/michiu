use crate::*;
use slotmap::{SecondaryMap, SparseSecondaryMap};



pub struct ContentStore {
    pub(crate) text_contents: SparseSecondaryMap<EntityId, std::borrow::Cow<'static, str>>,
    pub(crate) text_spans: SparseSecondaryMap<EntityId, Vec<TextSpan>>,
    pub(crate) input_contents: SparseSecondaryMap<EntityId, InputContents>,
    pub(crate) image_sources: SparseSecondaryMap<EntityId, ImageSource>,
    pub(crate) movie_properties: SparseSecondaryMap<EntityId, MovieProperty>,
    pub(crate) webview_contents: SparseSecondaryMap<EntityId, WebView2Contents>,
    
}

impl Default for ContentStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentStore {
    #[inline]
    pub fn new() -> Self {
        Self {
            text_contents: SparseSecondaryMap::new(),
            text_spans: SparseSecondaryMap::new(),
            input_contents: SparseSecondaryMap::new(),
            image_sources: SparseSecondaryMap::new(),
            movie_properties: SparseSecondaryMap::new(),
            webview_contents: SparseSecondaryMap::new(),
            
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.text_contents.clear();
        self.text_spans.clear();
        self.input_contents.clear();
        self.image_sources.clear();
        self.movie_properties.clear();
        self.webview_contents.clear();
        
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.text_contents.remove(id);
        self.text_spans.remove(id);
        self.input_contents.remove(id);
        self.image_sources.remove(id);
        self.movie_properties.remove(id);
        self.webview_contents.remove(id);
        
    }
}

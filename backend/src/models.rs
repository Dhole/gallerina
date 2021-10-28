// HTTP queries
pub mod queries {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub enum Sort {
        #[serde(rename = "name")]
        Name,
        #[serde(rename = "taken")]
        Taken,
        #[serde(rename = "modified")]
        Modified,
    }

    fn sort_default() -> Sort {
        Sort::Name
    }

    fn reverse_default() -> bool {
        true
    }

    #[derive(Debug, Deserialize)]
    pub struct FolderQuery {
        pub dir: String,
        #[serde(default = "sort_default")]
        pub sort: Sort,
        #[serde(default = "reverse_default")]
        pub reverse: bool,
    }

    #[derive(Debug, Deserialize)]
    pub struct ThumbQuery {
        pub path: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct SrcQuery {
        pub dir: String,
    }
}

// HTTP responses
pub mod responses {
    use serde::Serialize;

    use crate::models::views;
    use crate::scanner;

    #[derive(Debug, Serialize)]
    pub struct Folder {
        pub media: Vec<views::MediaData>,
        pub folders: Vec<views::FolderData>,
    }

    #[derive(Debug, Serialize)]
    pub struct ScannerReply {
        pub reply: scanner::Reply,
    }

    #[derive(Debug, Serialize)]
    pub struct Status {
        pub root: String,
        pub stats: scanner::Stats,
        pub scanner_state: scanner::State,
    }
}

// SQL views
pub mod views {
    use serde::Serialize;

    #[derive(Debug, Serialize, sqlx::FromRow)]
    pub struct MediaData {
        pub name: String,
    }

    #[derive(Debug, Serialize, sqlx::FromRow)]
    pub struct FolderData {
        pub name: String,
        pub media: Option<String>,
    }

    #[derive(Debug, sqlx::FromRow)]
    pub struct FolderScan {
        pub name: String,
        pub mtime: i64,
    }

    #[derive(Debug, sqlx::FromRow)]
    pub struct MediaScan {
        pub name: String,
        pub mtime: i64,
    }
}

// SQL tables
pub mod tables {
    #[derive(Debug, PartialEq, Eq, sqlx::FromRow)]
    pub struct Image {
        pub path: String,
        pub name: String,
        pub dir: String,
        pub mtime: i64,
        pub timestamp: i64,
    }

    #[derive(Debug, PartialEq, Eq, Hash, Clone, sqlx::FromRow)]
    pub struct Folder {
        pub path: String,
        pub name: String,
        pub dir: Option<String>,
        pub mtime: i64,
    }
}

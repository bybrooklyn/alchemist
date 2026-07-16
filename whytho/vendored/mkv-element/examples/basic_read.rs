use mkv_element::view::MatroskaView;
use remote_file::HttpFile;
use reqwest::Client;

#[tokio::main]
async fn main() {
    let file_url = "https://test-videos.co.uk/vids/jellyfish/mkv/720/Jellyfish_720_10s_5MB.mkv";
    let mut file = HttpFile::new(Client::new(), file_url).await.unwrap();
    let view = MatroskaView::new_async(&mut file).await.unwrap();
    let info = &view.segments[0].info;
    println!("Muxing app: {}", &*info.muxing_app);
    println!("Writing app: {}", &*info.writing_app);
}

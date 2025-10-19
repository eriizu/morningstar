pub fn get_best_matching_stop_name(stop_name: &str, stops: Vec<&str>) -> Option<String> {
    use fuse_rust::Fuse;
    let fuse = Fuse::default();
    let results = fuse.search_text_in_iterable(stop_name, stops.iter());
    results
        .iter()
        .reduce(|acc, item| if item.score < acc.score { item } else { acc })
        .map(|best_result| stops[best_result.index].to_owned())
}

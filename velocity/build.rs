fn main() {
    #[cfg(windows)]
    {
        embed_resource::compile("assets/velocity.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("failed to embed Windows resources");
    }
}

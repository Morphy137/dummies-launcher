fn main() {
    #[cfg(windows)]
    {
        let mut resource = winres::WindowsResource::new();
        resource.set_icon("assets/dummy.ico");
        resource.compile().expect("could not embed the dummy icon");
    }
}

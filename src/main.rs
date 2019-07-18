use nannou::prelude::*;
use itertools::Itertools;
use sample::{signal, Signal};

fn main() {
    process_audio_clip();

    nannou::app(model)
        .update(update)
        .simple_window(view)
        .run();
}

struct Model {}

fn model(_app: &App) -> Model {
    Model {}
}

fn update(_app: &App, _model: &mut Model, _update: Update) {
}

fn view(_app: &App, _model: &Model, frame: Frame) -> Frame {
    frame.clear(PURPLE);
    frame
}

fn process_audio_clip() {
    // Find and load the wav.
    let assets = find_folder::Search::ParentsThenKids(5, 5)
        .for_folder("assets")
        .unwrap();
    let reader = hound::WavReader::open(assets.join("clip.wav")).unwrap();
    let spec = reader.spec();
    println!("{:?}", spec);

    // Read the interleaved samples and convert them to a signal.
    let samples = reader.into_samples::<i16>().filter_map(Result::ok);
    let frames = signal::from_interleaved_samples_iter::<_, [i16; 2]>(samples).until_exhausted();

    // Find RMS value of audio samples for each video frame
    let frames_per_second = 1;
    let samples_per_video_frame = (spec.sample_rate / frames_per_second) as usize;
    for chunk in frames.chunks(samples_per_video_frame).into_iter() {
        let rms_for_video_frame = (chunk
            .map(|x| map_range((i16::min_value(), i16::max_value()), (-1.0, 1.0), x[0]))
            .fold(0.0, |acc, x| acc + x.powi(2))
            / samples_per_video_frame as f64)
            .sqrt();
        println!("{:?}", rms_for_video_frame);
    }
    println!("");
}

fn map_range(from_range: (i16, i16), to_range: (f64, f64), s: i16) -> f64 {
    to_range.0
        + (s as f64 - from_range.0 as f64) * (to_range.1 - to_range.0)
            / (from_range.1 as f64 - from_range.0 as f64)
}

#![deny(rust_2018_idioms)]

use itertools::Itertools;
use sample::{signal, Signal};

use image::ImageBuffer;
use image::Rgba;
use std::sync::Arc;
use vulkano::buffer::BufferUsage;
use vulkano::buffer::CpuAccessibleBuffer;
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::command_buffer::CommandBuffer;
use vulkano::command_buffer::DynamicState;
use vulkano::device::Device;
use vulkano::device::DeviceExtensions;
use vulkano::device::Features;
use vulkano::format::Format;
use vulkano::framebuffer::Framebuffer;
use vulkano::framebuffer::Subpass;
use vulkano::image::Dimensions;
use vulkano::image::StorageImage;
use vulkano::instance::Instance;
use vulkano::instance::InstanceExtensions;
use vulkano::instance::PhysicalDevice;
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::sync::GpuFuture;

use hound;

fn main() {
    let instance =
        Instance::new(None, &InstanceExtensions::none(), None).expect("failed to create instance");

    let physical = PhysicalDevice::enumerate(&instance)
        .next()
        .expect("no device available");

    let queue_family = physical
        .queue_families()
        .find(|&q| q.supports_graphics())
        .expect("couldn't find a graphical queue family");

    let (device, mut queues) = {
        Device::new(
            physical,
            &Features::none(),
            &DeviceExtensions::none(),
            [(queue_family, 0.5)].iter().cloned(),
        )
        .expect("failed to create device")
    };

    let queue = queues.next().unwrap();

    let image = StorageImage::new(
        device.clone(),
        Dimensions::Dim2d {
            width: 1024,
            height: 1024,
        },
        Format::R8G8B8A8Unorm,
        Some(queue.family()),
    )
    .unwrap();

    let buf = CpuAccessibleBuffer::from_iter(
        device.clone(),
        BufferUsage::all(),
        (0..1024 * 1024 * 4).map(|_| 0u8),
    )
    .expect("failed to create buffer");

    #[derive(Default, Copy, Clone)]
    struct Vertex {
        position: [f32; 2],
    }
    vulkano::impl_vertex!(Vertex, position);

    let vertex1 = Vertex {
        position: [-0.5, -0.5],
    };
    let vertex2 = Vertex {
        position: [0.0, 0.5],
    };
    let vertex3 = Vertex {
        position: [0.5, -0.25],
    };
    let vertex_buffer = CpuAccessibleBuffer::from_iter(
        device.clone(),
        BufferUsage::all(),
        vec![vertex1, vertex2, vertex3].into_iter(),
    )
    .unwrap();

    let render_pass = Arc::new(
        vulkano::single_pass_renderpass!(device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: Format::R8G8B8A8Unorm,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        )
        .unwrap(),
    );

    let framebuffer = Arc::new(
        Framebuffer::start(render_pass.clone())
            .add(image.clone())
            .unwrap()
            .build()
            .unwrap(),
    );

    mod vs {
        vulkano_shaders::shader! {
            ty: "vertex",
            src: "
#version 450
layout(location = 0) in vec2 position;
void main() {
    gl_Position = vec4(position, 0.0, 1.0);
}"
        }
    }

    mod fs {
        vulkano_shaders::shader! {
            ty: "fragment",
            src: "
#version 450
layout(location = 0) out vec4 f_color;
void main() {
    f_color = vec4(1.0, 0.0, 0.0, 1.0);
}"
        }
    }

    let vs = vs::Shader::load(device.clone()).expect("failed to create shader module");
    let fs = fs::Shader::load(device.clone()).expect("failed to create shader module");

    let pipeline = Arc::new(
        GraphicsPipeline::start()
            .vertex_input_single_buffer::<Vertex>()
            .vertex_shader(vs.main_entry_point(), ())
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs.main_entry_point(), ())
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .build(device.clone())
            .unwrap(),
    );

    let dynamic_state = DynamicState {
        viewports: Some(vec![Viewport {
            origin: [0.0, 0.0],
            dimensions: [1024.0, 1024.0],
            depth_range: 0.0..1.0,
        }]),
        ..DynamicState::none()
    };

    let command_buffer =
        AutoCommandBufferBuilder::primary_one_time_submit(device.clone(), queue.family())
            .unwrap()
            .begin_render_pass(
                framebuffer.clone(),
                false,
                vec![[0.0, 0.0, 1.0, 1.0].into()],
            )
            .unwrap()
            .draw(
                pipeline.clone(),
                &dynamic_state,
                vertex_buffer.clone(),
                (),
                (),
            )
            .unwrap()
            .end_render_pass()
            .unwrap()
            .copy_image_to_buffer(image.clone(), buf.clone())
            .unwrap()
            .build()
            .unwrap();

    let finished = command_buffer.execute(queue.clone()).unwrap();
    finished
        .then_signal_fence_and_flush()
        .unwrap()
        .wait(None)
        .unwrap();

    let buffer_content = buf.read().unwrap();
    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(1024, 1024, &buffer_content[..]).unwrap();
    image.save("image.png").unwrap();
}

/*
fn main() {
    process_audio_clip();
}

fn process_audio_clip() {
    // Find and load the wav.
    let assets = find_folder::Search::ParentsThenKids(5, 5)
        .for_folder("assets")
        .unwrap();
    let reader = hound::WavReader::open(assets.join("matt_clip.wav")).unwrap();
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
*/

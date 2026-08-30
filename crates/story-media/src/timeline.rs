use crate::MediaToolError;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct TimelineClip {
    pub input: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfmpegPlan {
    pub args: Vec<String>,
}

pub fn compile_concat_plan(
    ffmpeg: &Path,
    clips: &[TimelineClip],
    output: &Path,
) -> Result<FfmpegPlan, MediaToolError> {
    if !ffmpeg.is_absolute() || !output.is_absolute() || clips.is_empty() {
        return Err(MediaToolError::InvalidPath);
    }
    let mut args = Vec::with_capacity(clips.len() * 6 + 4);
    for clip in clips {
        let input = Path::new(&clip.input);
        if clip.input.trim().is_empty()
            || !input.is_absolute()
            || input
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
            || !clip.start_seconds.is_finite()
            || !clip.end_seconds.is_finite()
            || clip.start_seconds < 0.0
            || clip.end_seconds <= clip.start_seconds
            || clip.end_seconds - clip.start_seconds > 300.0
        {
            return Err(MediaToolError::InvalidArgument);
        }
        args.extend(["-i".into(), clip.input.clone()]);
    }
    args.extend([
        "-filter_complex".into(),
        build_filter(clips),
        "-map".into(),
        "[vout]".into(),
        "-an".into(),
        output.to_string_lossy().into_owned(),
    ]);
    Ok(FfmpegPlan { args })
}

fn build_filter(clips: &[TimelineClip]) -> String {
    let mut filter = String::new();
    for (index, clip) in clips.iter().enumerate() {
        filter.push_str(&format!(
            "[{index}:v]trim=start={}:end={},setpts=PTS-STARTPTS[v{index}];",
            clip.start_seconds, clip.end_seconds
        ));
    }
    let labels: String = (0..clips.len()).map(|i| format!("[v{i}]")).collect();
    filter.push_str(&format!("{labels}concat=n={}:v=1:a=0[vout]", clips.len()));
    filter
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn compiles_crop_and_supplement_clips_in_order() {
        let plan = compile_concat_plan(
            &PathBuf::from("C:\\tools\\ffmpeg.exe"),
            &[
                TimelineClip {
                    input: "C:\\input\\coarse.mp4".into(),
                    start_seconds: 1.0,
                    end_seconds: 4.0,
                },
                TimelineClip {
                    input: "C:\\input\\supplement.mp4".into(),
                    start_seconds: 0.0,
                    end_seconds: 2.0,
                },
            ],
            &PathBuf::from("C:\\out\\final.mp4"),
        )
        .unwrap();
        assert_eq!(plan.args[0], "-i");
        assert!(plan.args.iter().any(|arg| arg.contains("concat=n=2")));
        assert_eq!(plan.args.last().unwrap(), "C:\\out\\final.mp4");
    }

    #[test]
    fn rejects_invalid_ranges_and_relative_tool_paths() {
        let clip = TimelineClip {
            input: "C:\\input\\coarse.mp4".into(),
            start_seconds: 3.0,
            end_seconds: 2.0,
        };
        assert_eq!(
            compile_concat_plan(
                &PathBuf::from("ffmpeg"),
                &[clip.clone()],
                &PathBuf::from("C:\\out.mp4")
            ),
            Err(MediaToolError::InvalidPath)
        );
        assert_eq!(
            compile_concat_plan(
                &PathBuf::from("C:\\ffmpeg.exe"),
                &[clip],
                &PathBuf::from("C:\\out.mp4")
            ),
            Err(MediaToolError::InvalidArgument)
        );
        let relative = TimelineClip {
            input: "coarse.mp4".into(),
            start_seconds: 0.0,
            end_seconds: 1.0,
        };
        assert_eq!(
            compile_concat_plan(
                &PathBuf::from("C:\\ffmpeg.exe"),
                &[relative],
                &PathBuf::from("C:\\out.mp4")
            ),
            Err(MediaToolError::InvalidArgument)
        );
    }
}

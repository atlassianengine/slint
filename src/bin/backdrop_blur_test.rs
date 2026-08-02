slint::slint! {
    export component BackdropBlurTest inherits Window {
        preferred-width: 1280px;
        preferred-height: 760px;
        min-width: 800px;
        min-height: 520px;
        title: "Slint public backdrop-blur integration test";
        background: #0d1220;

        in-out property <float> phase: 0;

        for index in 6 : Rectangle {
            x: Math.mod(root.phase * (17 + index * 3) + index * 211, root.width / 1px + 180) * 1px - 90px;
            y: Math.mod(root.phase * (11 + index * 2) + index * 137, root.height / 1px + 140) * 1px - 70px;
            width: Math.mod(index, 2) == 0 ? 150px : 92px;
            height: Math.mod(index, 2) == 0 ? 150px : 92px;
            border-radius: self.width / 2;
            background: Math.mod(index, 2) == 0 ? #4f7fe8 : #c14f98;
        }

        for index in 8 : Rectangle {
            x: 80px + index * 145px;
            y: 250px + Math.mod(index * 67, 210) * 1px;
            width: 86px;
            height: 250px;
            border-radius: 18px;
            background: #b7c2da35;
            Text {
                text: "LIVE " + (index + 1);
                color: #d8e1f5;
                font-size: 13px;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        Rectangle {
            x: 48px; y: 48px; width: 420px; height: 174px;
            border-radius: 30px; background-blur: 6px;
            background: #171c2b99; border-width: 2px; border-color: #dce5ff66;
            Text { text: "6px"; color: white; font-size: 24px; x: 24px; y: 20px; }
        }
        Rectangle {
            x: parent.width - 480px; y: 58px; width: 420px; height: 174px;
            border-radius: 30px; background-blur: 12px;
            background: #171c2b99; border-width: 2px; border-color: #dce5ff66;
            Text { text: "12px"; color: white; font-size: 24px; x: 24px; y: 20px; }
        }
        Rectangle {
            x: parent.width / 2 - 250px; y: parent.height / 2 - 105px;
            width: 500px; height: 210px;
            border-radius: 34px; background-blur: 18px;
            background: #171c2b99; border-width: 2px; border-color: #dce5ff66;
            Text { text: "18px — overlaps earlier panels"; color: white; font-size: 24px; x: 24px; y: 20px; }
        }
        Rectangle {
            x: parent.width - 500px; y: parent.height - 232px;
            width: 438px; height: 176px;
            border-radius: 30px; background-blur: 32px;
            background: #171c2b99; border-width: 2px; border-color: #dce5ff66;
            Text { text: "32px — half resolution"; color: white; font-size: 24px; x: 24px; y: 20px; }
        }
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let app = BackdropBlurTest::new()?;
    let weak = app.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(16),
        move || {
            if let Some(app) = weak.upgrade() {
                app.set_phase(app.get_phase() + 0.55);
            }
        },
    );
    app.run()
}

# velomania
Allows to control BLE enabled indoor trainer (like zwift), but for free and without cartoon graphics!

# Run
```
cargo run -p backend -- --ftp-base 300 --workout backend/workouts/12wk_ftp_base/week7/1.zwo
```

Under heavy development!
# OS Support
Currently tested only on Ubuntu

# ZWO
[ZWO file reference](https://github.com/h4l/zwift-workout-file-reference/blob/master/zwift_workout_file_tag_reference.md)
# TODO:
[ ] (doing) Support BLE fitness machine indoor trainer

[*] support for majority of ZWO files

[ ] Create a mock for ble client/peripheral

[ ] Use tui-rs

[ ] Use egui?

[ ] Go WASM!

[ ] Add support for walkingpad https://github.com/ph4r05/ph4-walkingpad

# Using the app
[ ] ubuntu q&d disable screen blanking: ```gsettings set org.gnome.desktop.session idle-delay 0```, get prev value first https://askubuntu.com/questions/177348/how-do-i-disable-the-screensaver-lock
# Docs
- Data format of characteristics: GATT_Specification_Supplement_v5.pdf
- Description of GATTS fitness machine profile: FTMS_v1.0.pdf
- Moar description: FTMP_v1.0.pdf



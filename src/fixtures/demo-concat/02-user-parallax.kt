{
  "kinotrain": 1,
  "project": {
    "format_version": 1,
    "settings": {
      "fps": 30,
      "width": 1280,
      "height": 720,
      "sample_rate": 48000
    },
    "media": [
      {
        "id": 1,
        "path": "detail.mp4"
      },
      {
        "id": 2,
        "path": "green.mp4"
      }
    ],
    "clips": [
      {
        "id": 1,
        "link_group": null,
        "kind": "video",
        "lane": 0,
        "start": 0,
        "duration": 720000,
        "source_in": 0,
        "media": 1,
        "gain_db": 0.0,
        "fx": [
          {
            "kind": "event_grid",
            "cols": 2,
            "rows": 2,
            "center": true
          },
          {
            "kind": "pan3d",
            "pos_x": 0.0,
            "pos_y": 0.0,
            "pos_z": 0.0,
            "rot_x": 0.0,
            "rot_y": 0.0,
            "rot_z": 0.0,
            "anchor_x": 0.5,
            "anchor_y": 0.5,
            "scale": 1.0
          },
          {
            "kind": "event_grid",
            "cols": 2,
            "rows": 2,
            "center": true
          },
          {
            "kind": "pan3d",
            "pos_x": 0.0,
            "pos_y": 0.0,
            "pos_z": 0.0,
            "rot_x": 0.0,
            "rot_y": 0.0,
            "rot_z": 0.0,
            "anchor_x": 0.5,
            "anchor_y": 0.5,
            "scale": 1.0
          }
        ],
        "fx_auto": [
          [],
          [
            {
              "param": "rot_y",
              "ease": true,
              "keys": [
                {
                  "t": 0,
                  "v": 28.0
                },
                {
                  "t": 360000,
                  "v": 0.0
                }
              ]
            },
            {
              "param": "pos_x",
              "ease": true,
              "keys": [
                {
                  "t": 0,
                  "v": 90.0
                },
                {
                  "t": 360000,
                  "v": 0.0
                }
              ]
            }
          ],
          [],
          [
            {
              "param": "rot_y",
              "ease": true,
              "keys": [
                {
                  "t": 0,
                  "v": -14.0
                },
                {
                  "t": 360000,
                  "v": 0.0
                }
              ]
            },
            {
              "param": "scale",
              "ease": true,
              "keys": [
                {
                  "t": 0,
                  "v": 1.0
                },
                {
                  "t": 360000,
                  "v": 1.0
                },
                {
                  "t": 675000,
                  "v": 4.0
                }
              ]
            }
          ]
        ]
      }
    ],
    "tracks": [
      {
        "id": 1,
        "kind": "video"
      },
      {
        "id": 2,
        "kind": "audio"
      }
    ],
    "overlap_layers": true
  }
}

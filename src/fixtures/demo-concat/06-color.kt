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
        "duration": 360000,
        "source_in": 0,
        "media": 1,
        "gain_db": 0.0,
        "fx": [
          {
            "kind": "color_grade",
            "brightness": 0.05,
            "contrast": 0.35,
            "saturation": 1.5,
            "hue": -20.0
          }
        ]
      },
      {
        "id": 2,
        "link_group": null,
        "kind": "video",
        "lane": 0,
        "start": 360000,
        "duration": 360000,
        "source_in": 0,
        "media": 1,
        "gain_db": 0.0,
        "fx": [
          {
            "kind": "duotone",
            "shadow_r": 24,
            "shadow_g": 18,
            "shadow_b": 64,
            "high_r": 255,
            "high_g": 236,
            "high_b": 190,
            "amount": 1.0
          }
        ]
      },
      {
        "id": 3,
        "link_group": null,
        "kind": "video",
        "lane": 0,
        "start": 720000,
        "duration": 360000,
        "source_in": 0,
        "media": 1,
        "gain_db": 0.0,
        "fx": [
          {
            "kind": "posterize",
            "levels": 4
          }
        ]
      },
      {
        "id": 4,
        "link_group": null,
        "kind": "video",
        "lane": 0,
        "start": 1080000,
        "duration": 360000,
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
            "kind": "vignette",
            "strength": 0.5,
            "radius": 0.7,
            "softness": 0.45
          },
          {
            "kind": "geometry",
            "zoom": 2.0
          }
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

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
            "kind": "sharpen",
            "amount": 2.0,
            "radius": 3
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
            "kind": "pixelate",
            "size": 16
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
            "kind": "chromatic_aberration",
            "amount": 0.02
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
            "kind": "radial_blur",
            "amount": 0.5
          }
        ]
      },
      {
        "id": 5,
        "link_group": null,
        "kind": "video",
        "lane": 0,
        "start": 1440000,
        "duration": 360000,
        "source_in": 0,
        "media": 2,
        "gain_db": 0.0,
        "fx": [
          {
            "kind": "greenscreen",
            "key_r": 0,
            "key_g": 255,
            "key_b": 0,
            "tolerance": 90,
            "softness": 20
          },
          {
            "kind": "drop_shadow",
            "offset_x": 14,
            "offset_y": 14,
            "radius": 6,
            "opacity": 0.7
          }
        ]
      },
      {
        "id": 6,
        "link_group": null,
        "kind": "video",
        "lane": 0,
        "start": 1800000,
        "duration": 360000,
        "source_in": 0,
        "media": 1,
        "gain_db": 0.0,
        "fx": [
          {
            "kind": "halftone",
            "size": 10,
            "angle": 45.0
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

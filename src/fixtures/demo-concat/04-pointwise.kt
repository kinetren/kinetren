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
        "media": 2,
        "gain_db": 0.0,
        "fx": [
          {
            "kind": "event_grid",
            "cols": 2,
            "rows": 2,
            "center": true
          },
          {
            "kind": "greenscreen",
            "key_r": 0,
            "key_g": 255,
            "key_b": 0,
            "tolerance": 90,
            "softness": 20
          },
          {
            "kind": "geometry",
            "zoom": 2.0
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
            "kind": "event_grid",
            "cols": 2,
            "rows": 2,
            "center": true
          },
          {
            "kind": "noise",
            "amount": 0.35,
            "size": 2,
            "color": true
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

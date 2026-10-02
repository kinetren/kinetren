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
            "kind": "ripple",
            "amount": 0.12,
            "freq": 12.0
          }
        ],
        "fx_auto": [
          [
            {
              "param": "phase",
              "ease": true,
              "keys": [
                {
                  "t": 0,
                  "v": 0.0
                },
                {
                  "t": 360000,
                  "v": 1080.0
                }
              ]
            }
          ]
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
            "kind": "corner_pin"
          }
        ],
        "fx_auto": [
          [
            {
              "param": "x0",
              "ease": true,
              "keys": [
                {
                  "t": 0,
                  "v": 0.35
                },
                {
                  "t": 270000,
                  "v": 0.0
                }
              ]
            },
            {
              "param": "y0",
              "ease": true,
              "keys": [
                {
                  "t": 0,
                  "v": 0.25
                },
                {
                  "t": 270000,
                  "v": 0.0
                }
              ]
            },
            {
              "param": "x3",
              "ease": true,
              "keys": [
                {
                  "t": 0,
                  "v": 0.8
                },
                {
                  "t": 270000,
                  "v": 1.0
                }
              ]
            },
            {
              "param": "y3",
              "ease": true,
              "keys": [
                {
                  "t": 0,
                  "v": 0.75
                },
                {
                  "t": 270000,
                  "v": 1.0
                }
              ]
            }
          ]
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
            "kind": "lens_distort",
            "k1": -0.5
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
            "kind": "twirl",
            "angle": 120.0,
            "radius": 0.8
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
        "media": 1,
        "gain_db": 0.0,
        "fx": [
          {
            "kind": "kaleidoscope",
            "segments": 6
          },
          {
            "kind": "geometry",
            "zoom": 1.5
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
            "kind": "cylinder3d",
            "curvature": 0.8
          },
          {
            "kind": "geometry",
            "zoom": 1.3
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

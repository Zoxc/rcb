use term::Viewable;

use crate::term::{self, View};

struct Instance {
    iterations: usize,
    time_total: f64,
    count: usize,
}

impl Instance {
    fn avg(&self) -> Option<f64> {
        if self.count > 0 {
            Some(self.time_total / (self.count as f64))
        } else {
            None
        }
    }
}

fn print_values(values: &[f64], view: &mut View) {
    let width = 32;
    let pad = if view.col() < width {
        width - view.col()
    } else {
        0
    };
    format!("{:1$}", "", pad).view(view);

    let first = *values.first().unwrap();

    for (i, avg) in values.iter().enumerate() {
        term::color(100, 162, 217).view(view);
        format!("{:>8.04}s", avg).view(view);
        term::default_color().view(view);

        if i > 0 {
            let change = 100.0 * ((avg / first) - 1.0);

            if change > 0.5 {
                term::color(219, 126, 94).view(view);
            } else if change < -0.5 {
                term::color(143, 209, 98).view(view);
            }

            format!(" {:+6.02}%", change).view(view);
            term::default_color().view(view);
        }

        if i != values.len() - 1 {
            " ".view(view);
        }
    }
}

struct ConfigInstances {
    config: super::Config,
    builds: Vec<Instance>,
    completed: bool,
    started: bool,
}

impl ConfigInstances {
    fn avgs(&self, view: &mut View) {
        let avgs: Option<Vec<f64>> = self.builds.iter().map(|instance| instance.avg()).collect();
        avgs.map(|avgs| print_values(&avgs, view));
    }
}

pub struct Display {
    view: View,
    configs: Vec<ConfigInstances>,
    new_line_first: bool,
}

impl Display {
    pub(crate) fn new(configs: &Vec<super::ConfigInstances>, iterations: usize) -> Self {
        Display {
            new_line_first: true,
            view: View::new(),
            configs: configs
                .iter()
                .map(|config| ConfigInstances {
                    completed: false,
                    started: false,
                    config: config.config.clone(),
                    builds: config
                        .builds
                        .iter()
                        .map(|_| Instance {
                            iterations,
                            time_total: 0.0,
                            count: 0,
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    pub fn start_config(&mut self, config_index: usize) {
        self.configs[config_index].started = true;
        self.refresh();
    }

    pub fn report(&mut self, config_index: usize, build_index: usize, result: f64) {
        let mut config = &mut self.configs[config_index];
        let mut instance = &mut config.builds[build_index];
        instance.count += 1;
        instance.time_total += result;

        let complete = config
            .builds
            .iter()
            .all(|instance| instance.count == instance.iterations);

        if complete {
            config.completed = true;
            self.view.rewind();

            if self.new_line_first {
                term::newline().view(&mut self.view);
                self.new_line_first = false;
            }

            config.config.view(&mut self.view);
            " ".view(&mut self.view);
            config.avgs(&mut self.view);
            term::newline().view(&mut self.view);

            self.view.reset();
        }
        self.refresh();
    }

    pub fn refresh(&mut self) {
        self.view.rewind();

        term::newline().view(&mut self.view);

        "Running benchmarks:".view(&mut self.view);

        term::newline().view(&mut self.view);

        for config in self
            .configs
            .iter()
            .filter(|config| config.started && !config.completed)
        {
            let count: usize = config.builds.iter().map(|instance| instance.count).sum();
            let total: usize = config
                .builds
                .iter()
                .map(|instance| instance.iterations)
                .sum();

            " - ".view(&mut self.view);
            config.config.view(&mut self.view);
            format!(" ({}/{}) ", count, total).view(&mut self.view);
            config.avgs(&mut self.view);
            term::newline().view(&mut self.view);
        }
        self.view.flush();
    }

    pub fn complete(&mut self) {
        self.view.rewind();

        let builds = self.configs[0].builds.len();
        let totals: Vec<f64> = (0..builds)
            .map(|build| {
                self.configs
                    .iter()
                    .map(|config| config.builds[build].avg().unwrap())
                    .sum::<f64>()
            })
            .collect();

        term::newline().view(&mut self.view);

        "Total ".view(&mut self.view);
        print_values(&totals, &mut self.view);
        term::newline().view(&mut self.view);

        if builds > 1 {
            let summary: Vec<f64> = (0..builds)
                .map(|build| {
                    let instance_rel_sum = self
                        .configs
                        .iter()
                        .map(|config| config.builds[build].time_total / config.builds[0].time_total)
                        .sum::<f64>();
                    instance_rel_sum / (builds as f64)
                })
                .collect();

            "Summary ".view(&mut self.view);
            print_values(&summary, &mut self.view);
            term::newline().view(&mut self.view);
        }

        term::newline().view(&mut self.view);

        self.view.flush();
    }
}

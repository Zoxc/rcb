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

struct ConfigInstances {
    config: super::Config,
    builds: Vec<Instance>,
    completed: bool,
    started: bool,
}

impl ConfigInstances {
    fn avgs(&self, view: &mut View) {
        let avgs: Option<Vec<f64>> = self.builds.iter().map(|instance| instance.avg()).collect();

        let avgs = if let Some(avgs) = avgs { avgs } else { return };

        let width = 32;
        let pad = if view.col() < width {
            width - view.col()
        } else {
            0
        };
        format!("{:1$}", "", pad).view(view);

        let first = *avgs.first().unwrap();

        for (i, avg) in avgs.iter().enumerate() {
            term::color(100, 162, 217).view(view);
            format!("{:>8.04}s", avg).view(view);
            term::default_color().view(view);

            if i > 0 {
                let change = (avg / first) - 1.0;

                if change > 0.01 {
                    term::color(219, 126, 94).view(view);
                } else if change < -0.01 {
                    term::color(143, 209, 98).view(view);
                }

                format!(" {:+6.02}%", change * 100.0).view(view);
                term::default_color().view(view);
            }

            if i != avgs.len() - 1 {
                " ".view(view);
            }
        }
    }
}

pub struct Display {
    view: View,
    configs: Vec<ConfigInstances>,
}

impl Display {
    pub(crate) fn new(configs: &Vec<super::ConfigInstances>, iterations: usize) -> Self {
        Display {
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

            term::color(145, 145, 145).view(&mut self.view);
            "Completed ".view(&mut self.view);
            term::default_color().view(&mut self.view);
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
}

use bevy::{
  prelude::Material,
  reflect::TypeUuid,
  render::render_resource::{AsBindGroup, BindGroupLayoutEntry},
};

pub enum MaterialBuilder {
  PuzzleBool(String, u32, u32, spadina_core::asset::Transition),
  PuzzleNum(String, u32, Vec<u32>, spadina_core::asset::Transition),
  SettingBool(String, u32, u32, spadina_core::asset::Transition),
  SettingNum(String, u32, Vec<u32>, spadina_core::asset::Transition),
}

#[derive(TypeUuid, Clone, Debug)]
#[uuid = "ad80965f-48c3-4da8-8fbc-2a8236f74985"]
struct AestheticMaterial {
  aesthetic: spadina_core::asset::Aesthetic,
  material: spadina_core::asset::Material<
    std::sync::Arc<std::sync::Mutex<bevy::render::color::Color>>,
    std::sync::Arc<std::sync::Mutex<f64>>,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
  >,
}
impl Material for AestheticMaterial {
  fn vertex_shader() -> bevy::render::render_resource::ShaderRef {
    bevy::render::render_resource::ShaderRef::Default
  }

  fn fragment_shader() -> bevy::render::render_resource::ShaderRef {
    bevy::render::render_resource::ShaderRef::Default
  }

  fn specialize(
    pipeline: &bevy::pbr::MaterialPipeline<Self>,
    descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
    layout: &bevy::render::mesh::MeshVertexBufferLayout,
    key: bevy::pbr::MaterialPipelineKey<Self>,
  ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
    Ok(())
  }
}
impl AsBindGroup for AestheticMaterial {
  type Data;

  fn as_bind_group(
    &self,
    layout: &bevy::render::render_resource::BindGroupLayout,
    render_device: &bevy::render::renderer::RenderDevice,
    images: &bevy::render::render_asset::RenderAssets<bevy::prelude::Image>,
    fallback_image: &bevy::render::texture::FallbackImage,
  ) -> Result<bevy::render::render_resource::PreparedBindGroup<Self>, bevy::render::render_resource::AsBindGroupError> {
    todo!()
  }

  fn bind_group_layout(render_device: &bevy::render::renderer::RenderDevice) -> bevy::render::render_resource::BindGroupLayout {
    render_device.create_bind_group_layout(&bevy::render::render_resource::BindGroupLayoutDescriptor {
      label: None,
      entries: &[BindGroupLayoutEntry { binding: todo!(), visibility: todo!(), ty: todo!(), count: todo!() }],
    })
  }
}

impl MaterialBuilder {
  pub fn define(&mut self, x: u32, y: u32, z: u32) -> bevy::asset::Handle<bevy::pbr::StandardMaterial> {
    todo!()
  }
}
